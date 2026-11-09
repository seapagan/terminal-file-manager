//! At most one clipboard command runs outside the UI event loop.
use super::{ClipboardError, Delivery};
use crate::path_copy::CopyRequest;
use std::{
    io::Write,
    sync::mpsc::{self, Receiver, TryRecvError},
};

#[derive(Default)]
pub struct Runtime {
    pending: Option<(String, Receiver<Result<Delivery, ClipboardError>>)>,
}

impl Runtime {
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub fn start(&mut self, request: CopyRequest, fallback: bool) -> Result<(), String> {
        self.start_using(request, move |text| super::copy(text.as_bytes(), fallback))
    }

    fn start_using(
        &mut self,
        request: CopyRequest,
        copy: impl FnOnce(String) -> Result<Delivery, ClipboardError> + Send + 'static,
    ) -> Result<(), String> {
        if self.busy() {
            return Err("Clipboard request still running; wait before copying again".into());
        }
        let (send, receive) = mpsc::channel();
        std::thread::Builder::new()
            .name("tfm-clipboard".into())
            .spawn(move || {
                let _ = send.send(copy(request.text));
            })
            .map_err(|e| format!("Cannot start clipboard worker: {e}"))?;
        self.pending = Some((request.scope, receive));
        Ok(())
    }

    /// The UI thread emits terminal output serially between frame draws.
    pub fn poll(&mut self, terminal: &mut dyn Write) -> Option<Result<String, String>> {
        let (_, receive) = self.pending.as_ref()?;
        let result = match receive.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(ClipboardError(
                "Clipboard worker stopped without a result".into(),
            )),
        };
        let (scope, _) = self.pending.take().expect("pending result");
        Some(
            result
                .map_err(|e| e.to_string())
                .and_then(|delivery| finish(delivery, &scope, terminal)),
        )
    }
}

fn finish(delivery: Delivery, scope: &str, terminal: &mut dyn Write) -> Result<String, String> {
    match delivery {
        Delivery::Copied => Ok(format!("Copied {scope}")),
        Delivery::Terminal(sequence) => {
            terminal
                .write_all(&sequence)
                .and_then(|()| terminal.flush())
                .map_err(|e| format!("Cannot send OSC 52 to terminal: {e}"))?;
            Ok(format!(
                "Sent {scope} via OSC 52; terminal acceptance unconfirmed"
            ))
        }
    }
}

#[cfg(test)]
mod tests;
