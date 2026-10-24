//! Native driver adapted from Labeldeck 8c0fdea tests/common/terminal.rs (MIT, Grant Ramsay).
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
    io::{self, Read, Write},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const TIMEOUT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(20);

pub fn drive(command: CommandBuilder, steps: &[(&str, &str)], resize: bool) -> String {
    let mut transcript = Vec::new();
    let result = Session::open(command).and_then(|mut session| {
        session.drive(steps, Instant::now() + TIMEOUT, &mut transcript, resize)
    });
    let text = String::from_utf8_lossy(&transcript).into_owned();
    assert!(
        result.is_ok(),
        "PTY session failed: {result:?}; transcript: {:?}",
        text.chars().take(16000).collect::<String>()
    );
    assert!(
        contains(&transcript, b"\x1b[?2004l"),
        "paste mode was not restored"
    );
    if contains(&transcript, b"\x1b[?1006h") {
        assert!(
            contains(&transcript, b"\x1b[?1006l"),
            "mouse capture was not restored"
        );
    }
    if contains(&transcript, b"\x1b[>1u") {
        assert!(
            contains(&transcript, b"\x1b[<1u"),
            "keyboard protocol was not popped"
        );
    }
    text
}

pub struct Session {
    pub(crate) child: Box<dyn Child + Send + Sync>,
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    reader: Option<JoinHandle<()>>,
    output: Receiver<io::Result<Vec<u8>>>,
    // Keep recv_timeout usable after the reader sends EOF and exits.
    _sender: mpsc::Sender<io::Result<Vec<u8>>>,
    exited: bool,
}

impl Session {
    pub fn open(command: CommandBuilder) -> io::Result<Self> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 100,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(io::Error::other)?;
        let reader = pair.master.try_clone_reader().map_err(io::Error::other)?;
        let writer = pair.master.take_writer().map_err(io::Error::other)?;
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(io::Error::other)?;
        drop(pair.slave);
        let (sender, output) = mpsc::channel();
        let mut session = Self {
            child,
            master: Some(pair.master),
            writer: Some(writer),
            reader: None,
            output,
            _sender: sender.clone(),
            exited: false,
        };
        session.reader = Some(thread::Builder::new().spawn(move || {
            read_output(reader, sender);
        })?);
        Ok(session)
    }

    pub fn drive(
        &mut self,
        steps: &[(&str, &str)],
        deadline: Instant,
        transcript: &mut Vec<u8>,
        resize: bool,
    ) -> io::Result<()> {
        let mut script = Script::default();
        let mut eof = false;
        let mut status = None;
        while !eof || status.is_none() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "terminal driver timed out; next step: {:?}; last sent: {:?}",
                        steps.get(script.step),
                        script
                            .step
                            .checked_sub(1)
                            .and_then(|index| steps.get(index))
                    ),
                ));
            }
            eof |= self.read_step(remaining.min(POLL), &mut script, steps, transcript, resize)?;
            if status.is_none() {
                status = self.child.try_wait()?;
                if status.is_some() {
                    self.exited = true;
                    // ConPTY retains its output pipe until the console closes.
                    // The reader keeps draining while ClosePseudoConsole runs.
                    self.master.take();
                }
            }
        }
        script.finish(steps, &status.unwrap(), transcript)
    }
    pub fn read_step(
        &mut self,
        wait: Duration,
        script: &mut Script,
        steps: &[(&str, &str)],
        transcript: &mut Vec<u8>,
        resize: bool,
    ) -> io::Result<bool> {
        match self.output.recv_timeout(wait) {
            Ok(chunk) => {
                let chunk = chunk?;
                transcript.extend_from_slice(&chunk);
                let previous = script.step;
                script.observe(&chunk, steps, self.writer.as_mut().unwrap())?;
                if resize
                    && script.step != previous
                    && let Some(master) = &self.master
                {
                    let (rows, cols) = if script.step == 1 {
                        (20, 65)
                    } else {
                        (30, 120)
                    };
                    master
                        .resize(PtySize {
                            rows,
                            cols,
                            pixel_width: 0,
                            pixel_height: 0,
                        })
                        .map_err(io::Error::other)?;
                }
                Ok(chunk.is_empty())
            }
            Err(RecvTimeoutError::Timeout) => Ok(false),
            Err(error) => Err(io::Error::other(error)),
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.exited {
            // Native Unix children are std::process::Child. Use its hard kill
            // rather than portable-pty's cloned killer, which only sends HUP.
            let child: &mut dyn Child = self.child.as_mut();
            if let Some(child) = child.downcast_mut::<std::process::Child>() {
                let _ = child.kill();
            } else {
                let _ = self.child.kill();
            }
            let _ = self.child.wait();
        }
        self.writer.take();
        self.master.take();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn read_output(mut reader: Box<dyn Read + Send>, sender: mpsc::Sender<io::Result<Vec<u8>>>) {
    let mut buffer = vec![0; 65536];
    loop {
        let chunk = match reader.read(&mut buffer) {
            Ok(size) => Ok(buffer[..size].to_vec()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                continue;
            }
            Err(error) => Err(error),
        };
        let done = chunk.as_ref().map_or(true, Vec::is_empty);
        if sender.send(chunk).is_err() || done {
            break;
        }
    }
}

#[derive(Default)]
pub struct Script {
    pending: Vec<u8>,
    query: [Vec<u8>; 3],
    pub step: usize,
    ansi: u8,
}

impl Script {
    fn finish(
        &self,
        steps: &[(&str, &str)],
        status: &portable_pty::ExitStatus,
        transcript: &[u8],
    ) -> io::Result<()> {
        if let Some((marker, _)) = steps.get(self.step) {
            return Err(io::Error::other(format!(
                "unreached terminal step: {marker}"
            )));
        }
        if !status.success() {
            return Err(io::Error::other(format!("child exit status {status}")));
        }
        if !contains(transcript, b"\x1b[?1049l") {
            return Err(io::Error::other("alternate screen was not restored"));
        }
        Ok(())
    }

    fn observe(
        &mut self,
        chunk: &[u8],
        steps: &[(&str, &str)],
        writer: &mut dyn Write,
    ) -> io::Result<()> {
        append_visible(&mut self.pending, &mut self.ansi, chunk);
        if self.pending.len() > 1_048_576 {
            return Err(io::Error::other("terminal marker buffer exceeded 1 MiB"));
        }
        let queries = [
            (b"\x1b[?u".as_slice(), b"\x1b[?0u".as_slice()),
            (b"\x1b[c".as_slice(), b"\x1b[?1;2c".as_slice()),
            (b"\x1b[6n".as_slice(), b"\x1b[1;1R".as_slice()),
        ];
        for (buffer, (query, response)) in self.query.iter_mut().zip(queries) {
            buffer.extend_from_slice(chunk);
            while let Some(index) = buffer
                .windows(query.len())
                .position(|window| window == query)
            {
                writer.write_all(response)?;
                buffer.drain(..index + query.len());
            }
            buffer.drain(..buffer.len().saturating_sub(query.len() - 1));
        }
        if let Some((marker, keys)) = steps.get(self.step)
            && contains(
                &self.pending,
                &marker
                    .bytes()
                    .filter(|b| !b.is_ascii_whitespace())
                    .collect::<Vec<_>>(),
            )
        {
            self.pending.clear();
            writer.write_all(keys.as_bytes())?;
            self.step += 1;
        }
        writer.flush()
    }
}

fn contains(bytes: &[u8], marker: &[u8]) -> bool {
    bytes.windows(marker.len()).any(|window| window == marker)
}

fn append_visible(text: &mut Vec<u8>, ansi: &mut u8, chunk: &[u8]) {
    for byte in chunk {
        match *ansi {
            0 if *byte == 0x1b => *ansi = 1,
            0 => {
                if !byte.is_ascii_whitespace() {
                    text.push(*byte);
                }
            }
            1 => *ansi = if *byte == b'[' { 2 } else { 0 },
            _ => {
                if (0x40..=0x7e).contains(byte) {
                    *ansi = 0;
                }
            }
        }
    }
}
