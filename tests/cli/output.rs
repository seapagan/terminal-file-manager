use super::*;

#[test]
fn help_and_version_exit_cleanly_when_the_stdout_pipe_is_closed() {
    let root = tempfile::tempdir().unwrap();
    for argument in ["--help", "-h", "--version", "-V"] {
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        let output = isolated_session_command(root.path())
            .arg(argument)
            .stdout(writer)
            .output()
            .unwrap();
        assert!(output.stderr.is_empty(), "{argument}: {:?}", output.stderr);
        assert!(output.status.success(), "{argument}: {}", output.status);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn stdout_write_failures_are_reported_without_a_panic() {
    let root = tempfile::tempdir().unwrap();
    let output = isolated_session_command(root.path())
        .arg("--help")
        .stdout(
            std::fs::File::options()
                .write(true)
                .open("/dev/full")
                .unwrap(),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("tfm: could not write to stdout: "),
        "{stderr}"
    );
    assert!(!stderr.contains("panicked"), "{stderr}");
}
