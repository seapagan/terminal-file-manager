use super::*;

#[test]
fn nonreading_provider_times_out_while_payload_exceeds_pipe_capacity() {
    let root = tempfile::tempdir().unwrap();
    let pid_file = root.path().join("descendant.pid");
    let mut child = spawn_provider("sleep 30 & printf '%s' \"$!\" > \"$1\"; wait", &pid_file);
    let ready = Instant::now() + Duration::from_secs(3);
    while !pid_file.exists() {
        assert!(
            Instant::now() < ready,
            "provider did not start its descendant"
        );
        std::thread::yield_now();
    }
    let error = drive(
        "hung",
        &mut child,
        &vec![b'x'; 1024 * 1024],
        Instant::now() + Duration::from_millis(100),
    )
    .unwrap_err();
    terminate(&mut child).unwrap();
    child.wait().unwrap();
    assert!(error.to_string().contains("timed out"));
    assert_descendant_terminated(&pid_file);
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
    let state = process_state(&pid.to_string());
    let _ = nix::sys::signal::kill(Pid::from_raw(pid), Signal::SIGKILL);
    assert!(result.is_ok(), "{result:?}");
    assert!(
        !state.trim().is_empty() && !state.trim().starts_with('Z'),
        "{state}"
    );
}

#[test]
fn failed_and_noisy_exits_clean_up_the_provider_descendants() {
    for outcome in ["exit 7", "printf noisy"] {
        let root = tempfile::tempdir().unwrap();
        let pid_file = root.path().join("descendant.pid");
        let script = format!("sleep 30 & printf '%s' \"$!\" > \"$1\"; cat >/dev/null; {outcome}");
        let result = run_provider(
            "failed",
            "/bin/sh",
            &["-c", &script, "sh", pid_file.to_str().unwrap()],
            b"text",
        );
        assert!(result.is_err(), "{result:?}");
        assert_descendant_terminated(&pid_file);
    }
}

#[test]
fn observing_an_exited_provider_retains_its_pid_until_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let mut child = spawn_provider("cat >/dev/null; exit 7", &root.path().join("unused"));
    assert!(
        drive(
            "failed",
            &mut child,
            b"text",
            Instant::now() + Duration::from_secs(3)
        )
        .is_err()
    );
    let observed = observe_exit(&child).unwrap();
    let repeated = observe_exit(&child).unwrap();
    terminate(&mut child).unwrap();
    let reaped = child.wait().unwrap();
    assert_eq!(observed, Some(reaped));
    assert_eq!(repeated, observed);
}

#[test]
fn observing_a_signaled_provider_preserves_status_without_reaping() {
    let root = tempfile::tempdir().unwrap();
    let mut child = spawn_provider(
        "cat >/dev/null; kill -TERM \"$$\"",
        &root.path().join("unused"),
    );
    assert!(
        drive(
            "signaled",
            &mut child,
            b"text",
            Instant::now() + Duration::from_secs(3)
        )
        .is_err()
    );
    let observed = observe_exit(&child).unwrap();
    assert_eq!(observe_exit(&child).unwrap(), observed);
    let reaped = child.wait().unwrap();
    assert_eq!(reaped.signal(), Some(Signal::SIGTERM as i32));
    assert_eq!(observed, Some(reaped));
}

#[test]
fn cleanup_rejects_a_child_whose_pid_was_already_reaped() {
    let mut child = Command::new("/bin/sh")
        .args(["-c", "exit 7"])
        .process_group(0)
        .spawn()
        .unwrap();
    assert_eq!(child.wait().unwrap().code(), Some(7));
    let error = terminate(&mut child).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(nix::errno::Errno::ECHILD as i32));
}

fn assert_descendant_terminated(pid_file: &std::path::Path) {
    let pid = std::fs::read_to_string(pid_file).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let state = process_state(&pid);
        if state.trim().is_empty() || state.trim().starts_with('Z') {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "descendant {pid} remains alive: {state}"
        );
        std::thread::yield_now();
    }
}

fn process_state(pid: &str) -> String {
    let state = Command::new("/bin/ps")
        .args(["-o", "stat=", "-p", pid])
        .output()
        .unwrap();
    assert!(
        state.status.success() || state.status.code() == Some(1),
        "{state:?}"
    );
    String::from_utf8(state.stdout).unwrap()
}

fn spawn_provider(script: &str, pid_file: &std::path::Path) -> Child {
    Command::new("/bin/sh")
        .args(["-c", script, "sh", pid_file.to_str().unwrap()])
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn early_stdin_close_preserves_failure_diagnostics_and_rejects_incomplete_success() {
    for (exit, expected) in [(7, "failed with exit status: 7"), (0, "complete payload")] {
        let script = format!("exec 0<&-; printf 'display unavailable' >&2; exit {exit}");
        let error = run_provider(
            "closed",
            "/bin/sh",
            &["-c", &script],
            &vec![b'x'; 1024 * 1024],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(expected), "{error}");
        if exit != 0 {
            assert!(error.contains("display unavailable"), "{error}");
        }
    }
}

#[test]
fn closed_stdin_does_not_disable_deadline_or_descendant_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let pid_file = root.path().join("descendant.pid");
    let error = run_with_deadline(
        "closed",
        "/bin/sh",
        &[
            "-c",
            "exec 0<&-; sleep 30 & printf '%s' \"$!\" > \"$1\"; wait",
            "sh",
            pid_file.to_str().unwrap(),
        ],
        &vec![b'x'; 1024 * 1024],
        Duration::from_millis(100),
    )
    .unwrap_err();
    assert!(error.to_string().contains("timed out"), "{error}");
    assert_descendant_terminated(&pid_file);
}
