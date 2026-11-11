//! Nonblocking bounded provider I/O; daemon children cannot retain helper threads.
use super::ClipboardError;
use nix::{
    fcntl::{FcntlArg, OFlag, fcntl},
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use std::{
    io::{Read, Write},
    os::{
        fd::AsFd,
        unix::process::{CommandExt, ExitStatusExt},
    },
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};

pub(super) fn run_provider(
    name: &str,
    program: &str,
    args: &[&str],
    payload: &[u8],
) -> Result<(), ClipboardError> {
    run_with_deadline(name, program, args, payload, Duration::from_secs(3))
}

fn run_with_deadline(
    name: &str,
    program: &str,
    args: &[&str],
    payload: &[u8],
    timeout: Duration,
) -> Result<(), ClipboardError> {
    let mut child = Command::new(program)
        .args(args)
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ClipboardError(format!("cannot start clipboard provider '{name}': {e}")))?;
    let result = drive(name, &mut child, payload, Instant::now() + timeout);
    if result.is_err() {
        let _ = terminate(&mut child);
    }
    let _ = child.wait();
    result
}

// WNOWAIT retains the leader PID until failure cleanup has signalled its group.
fn observe_exit(child: &Child) -> std::io::Result<Option<std::process::ExitStatus>> {
    use rustix::process::{Pid, WaitId, WaitIdOptions, waitid};
    let options = WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT;
    loop {
        match waitid(
            WaitId::Pid(Pid::from_raw(child.id() as i32).expect("child PID")),
            options,
        ) {
            Ok(status) => {
                return Ok(status.map(|status| {
                    let raw = status.exit_status().map_or_else(
                        || {
                            status.terminating_signal().expect("terminal waitid status")
                                | if status.dumped() { 0x80 } else { 0 }
                        },
                        |code| code << 8,
                    );
                    std::process::ExitStatus::from_raw(raw)
                }));
            }
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

fn terminate(child: &mut Child) -> std::io::Result<()> {
    // An already-reaped child no longer owns its PID; never signal that group.
    observe_exit(child)?;
    let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGKILL);
    let _ = child.kill();
    Ok(())
}

fn drive(
    name: &str,
    child: &mut Child,
    payload: &[u8],
    deadline: Instant,
) -> Result<(), ClipboardError> {
    let mut pipes = Pipes::new(child)?;
    let mut offset = 0;
    loop {
        pipes.write(payload, &mut offset).map_err(|e| {
            ClipboardError(format!("cannot write to clipboard provider '{name}': {e}"))
        })?;
        pipes.read()?;
        if let Some(status) = observe_exit(child).map_err(|e| {
            ClipboardError(format!("cannot wait for clipboard provider '{name}': {e}"))
        })? {
            pipes.read()?;
            return pipes.finish(name, status, offset == payload.len());
        }
        if Instant::now() >= deadline {
            return Err(ClipboardError(format!(
                "clipboard provider '{name}' timed out"
            )));
        }
        pipes.wait()?;
    }
}

struct Pipes {
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    stderr: ChildStderr,
    output: Vec<u8>,
    error: Vec<u8>,
}

impl Pipes {
    fn wait(&self) -> Result<(), ClipboardError> {
        if let Some(stdin) = &self.stdin {
            let mut fds = [nix::poll::PollFd::new(
                stdin.as_fd(),
                nix::poll::PollFlags::POLLOUT,
            )];
            match nix::poll::poll(&mut fds, 5u16) {
                Ok(_) | Err(nix::errno::Errno::EINTR) => {}
                Err(error) => {
                    return Err(ClipboardError(format!(
                        "Cannot wait for clipboard pipe: {error}"
                    )));
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
    fn new(child: &mut Child) -> Result<Self, ClipboardError> {
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        nonblocking(&stdin)?;
        nonblocking(&stdout)?;
        nonblocking(&stderr)?;
        Ok(Self {
            stdin: Some(stdin),
            stdout,
            stderr,
            output: Vec::new(),
            error: Vec::new(),
        })
    }

    fn write(&mut self, payload: &[u8], offset: &mut usize) -> std::io::Result<()> {
        if let Some(stdin) = &mut self.stdin {
            match stdin.write(&payload[*offset..]) {
                Ok(count) => *offset += count,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error),
            }
            if *offset == payload.len() {
                self.stdin = None;
            }
        }
        Ok(())
    }

    fn read(&mut self) -> Result<(), ClipboardError> {
        read_available(&mut self.stdout, &mut self.output)?;
        read_available(&mut self.stderr, &mut self.error)?;
        Ok(())
    }

    fn finish(
        &self,
        name: &str,
        status: std::process::ExitStatus,
        written: bool,
    ) -> Result<(), ClipboardError> {
        if !status.success() {
            return Err(ClipboardError(format!(
                "clipboard provider '{name}' failed with {status}: {}",
                String::from_utf8_lossy(&self.error)
            )));
        }
        if !written {
            return Err(ClipboardError(format!(
                "cannot write to clipboard provider '{name}': closed before reading the complete payload"
            )));
        }
        if !self.output.is_empty() {
            return Err(ClipboardError(format!(
                "clipboard provider '{name}' produced unexpected output"
            )));
        }
        Ok(())
    }
}

fn nonblocking(fd: &impl AsFd) -> Result<(), ClipboardError> {
    let flags = fcntl(fd, FcntlArg::F_GETFL)
        .map_err(|e| ClipboardError(format!("Cannot inspect clipboard pipe: {e}")))?;
    fcntl(
        fd,
        FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK),
    )
    .map_err(|e| ClipboardError(format!("Cannot configure clipboard pipe: {e}")))?;
    Ok(())
}

fn read_available(pipe: &mut impl Read, output: &mut Vec<u8>) -> Result<(), ClipboardError> {
    let mut buffer = [0; 1024];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(count) => output.extend_from_slice(&buffer[..count]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                return Ok(());
            }
            Err(error) => {
                return Err(ClipboardError(format!(
                    "Cannot read clipboard provider output: {error}"
                )));
            }
        }
        if output.len() > 8192 {
            return Err(ClipboardError(
                "Clipboard provider output exceeded 8192 bytes".into(),
            ));
        }
    }
}

#[cfg(all(test, unix))]
mod tests;
