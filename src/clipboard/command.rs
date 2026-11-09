//! Nonblocking bounded provider I/O; daemon children cannot retain helper threads.
use super::ClipboardError;
use nix::{
    fcntl::{FcntlArg, OFlag, fcntl},
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use std::{
    io::{Read, Write},
    os::{fd::AsFd, unix::process::CommandExt},
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
        // Only the dedicated provider group is terminated; successful clipboard daemons remain alive.
        let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGKILL);
        let _ = child.kill();
    }
    let _ = child.wait();
    result
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
        if let Some(status) = child.try_wait().map_err(|e| {
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
mod tests {
    use super::*;

    #[test]
    fn nonreading_provider_times_out_while_payload_exceeds_pipe_capacity() {
        let error = run_with_deadline(
            "hung",
            "/bin/sh",
            &["-c", "exec sleep 30"],
            &vec![b'x'; 1024 * 1024],
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
    }

    #[test]
    fn provider_receives_the_entire_maximum_payload_through_pipe_backpressure() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("clipboard.txt");
        let payload = vec![b'x'; super::super::MAX_BYTES];
        run_provider(
            "large",
            "/bin/sh",
            &["-c", "cat > \"$1\"", "sh", output.to_str().unwrap()],
            &payload,
        )
        .unwrap();
        assert_eq!(std::fs::read(output).unwrap(), payload);
    }

    #[test]
    fn unexpected_output_is_captured_instead_of_printed_over_the_tui() {
        let error = run_provider(
            "noisy",
            "/bin/sh",
            &["-c", "cat >/dev/null; printf '\\033[31mnoisy'"],
            b"literal",
        )
        .unwrap_err();
        assert!(error.to_string().contains("unexpected output"));
        let error = run_provider(
            "noisy",
            "/bin/sh",
            &["-c", "cat >/dev/null; head -c 9000 /dev/zero"],
            b"literal",
        )
        .unwrap_err();
        assert!(error.to_string().contains("8192"));
    }

    #[test]
    fn successful_daemon_parent_does_not_wait_for_inherited_output_pipe_eof() {
        let root = tempfile::tempdir().unwrap();
        let pid_file = root.path().join("daemon.pid");
        let result = run_with_deadline(
            "daemon",
            "/bin/sh",
            &[
                "-c",
                "cat >/dev/null; sleep 30 >/dev/null & printf '%s' \"$!\" > \"$1\"",
                "sh",
                pid_file.to_str().unwrap(),
            ],
            b"text",
            Duration::from_secs(3),
        );
        let pid: i32 = std::fs::read_to_string(pid_file).unwrap().parse().unwrap();
        let _ = nix::sys::signal::kill(Pid::from_raw(pid), Signal::SIGKILL);
        assert!(result.is_ok(), "{result:?}");
    }
}
