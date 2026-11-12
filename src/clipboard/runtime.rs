//! At most one clipboard command runs outside the UI event loop.
use super::{ClipboardError, Delivery};
use crate::path_copy::{BaseRequest, CopyRequest, ValidatedBase};
use std::{
    io::Write,
    sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError},
    time::Duration,
};

#[derive(Default)]
pub struct Runtime {
    pending: Option<(String, Receiver<Result<Work, String>>)>,
}

enum Work {
    Copy(Delivery),
    Base(ValidatedBase),
}

pub enum Completion {
    Message(String),
    Base(ValidatedBase),
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
        self.start_inspecting(request, crate::fs::inspect_identity, copy)
    }

    fn start_inspecting(
        &mut self,
        request: CopyRequest,
        inspect: impl FnMut(&std::path::Path) -> crate::fs::Entry + Send + 'static,
        copy: impl FnOnce(String) -> Result<Delivery, ClipboardError> + Send + 'static,
    ) -> Result<(), String> {
        let scope = request.scope.clone();
        self.spawn(scope, move || {
            let text = request.prepare_using(inspect)?;
            copy(text)
                .map(Work::Copy)
                .map_err(|error| error.to_string())
        })
    }

    pub fn start_base(&mut self, request: BaseRequest) -> Result<(), String> {
        self.start_base_inspecting(request, crate::fs::inspect_identity)
    }

    fn start_base_inspecting(
        &mut self,
        request: BaseRequest,
        inspect: impl FnMut(&std::path::Path) -> crate::fs::Entry + Send + 'static,
    ) -> Result<(), String> {
        self.spawn("relative-path base".into(), move || {
            request.prepare_using(inspect).map(Work::Base)
        })
    }

    fn spawn(
        &mut self,
        scope: String,
        work: impl FnOnce() -> Result<Work, String> + Send + 'static,
    ) -> Result<(), String> {
        if self.busy() {
            return Err("Clipboard request still running; wait before copying again".into());
        }
        let (send, receive) = mpsc::channel();
        std::thread::Builder::new()
            .name("tfm-clipboard".into())
            .spawn(move || {
                let _ = send.send(work());
            })
            .map_err(|e| format!("Cannot start clipboard worker: {e}"))?;
        self.pending = Some((scope, receive));
        Ok(())
    }

    /// The UI thread emits terminal output serially between frame draws.
    pub fn poll(&mut self, terminal: &mut dyn Write) -> Option<Result<Completion, String>> {
        let (_, receive) = self.pending.as_ref()?;
        let result = match receive.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                Err("Clipboard worker stopped without a result".into())
            }
        };
        let (scope, _) = self.pending.take().expect("pending result");
        Some(complete(result, &scope, terminal))
    }

    /// Drain accepted work before terminal teardown; never join a blocked worker.
    pub fn shutdown(
        &mut self,
        terminal: &mut dyn Write,
        timeout: Duration,
    ) -> Option<Result<Completion, String>> {
        let (scope, receive) = self.pending.take()?;
        let result = receive.recv_timeout(timeout).unwrap_or_else(|error| {
            Err(match error {
                RecvTimeoutError::Timeout => {
                    format!("Clipboard shutdown timed out for {scope}; completion unconfirmed")
                }
                RecvTimeoutError::Disconnected => {
                    "Clipboard worker stopped without a result".into()
                }
            })
        });
        Some(complete(result, &scope, terminal))
    }
}

fn complete(
    result: Result<Work, String>,
    scope: &str,
    terminal: &mut dyn Write,
) -> Result<Completion, String> {
    result.and_then(|work| match work {
        Work::Copy(delivery) => finish(delivery, scope, terminal).map(Completion::Message),
        Work::Base(request) => Ok(Completion::Base(request)),
    })
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
