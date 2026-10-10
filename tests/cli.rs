//! Smoke tests for the implemented executable boundary.

use std::process::{Command, Output};

fn tfm(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tfm"))
        .args(arguments)
        .output()
        .expect("tfm must execute")
}

#[test]
fn help_and_empty_arguments_describe_the_foundation() {
    for arguments in [&[][..], &["--help"][..], &["-h"][..]] {
        let output = tfm(arguments);
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).expect("help is UTF-8");
        assert!(text.contains("Usage: tfm"));
        assert!(text.contains("File management is not implemented yet."));
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn version_uses_the_executable_name_and_manifest_version() {
    for argument in ["--version", "-V"] {
        let output = tfm(&[argument]);
        assert!(output.status.success());
        assert_eq!(
            output.stdout,
            concat!("tfm ", env!("CARGO_PKG_VERSION"), "\n").as_bytes()
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn unsupported_arguments_fail_without_stdout() {
    for arguments in [&["path"][..], &["--unknown"][..], &["--help", "extra"][..]] {
        let output = tfm(arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"tfm: unsupported arguments; use --help\n");
    }
}
