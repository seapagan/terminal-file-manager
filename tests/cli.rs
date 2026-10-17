//! Smoke tests for the implemented executable boundary.

use std::process::{Command, Output};

fn tfm(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tfm"))
        .args(arguments)
        .output()
        .expect("tfm must execute")
}

#[test]
fn help_describes_the_read_only_browser() {
    for arguments in [&["--help"][..], &["-h"][..]] {
        let output = tfm(arguments);
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).expect("help is UTF-8");
        assert!(text.contains("Usage: tfm"));
        assert!(text.contains("read-only"));
        assert!(text.contains("DIRECTORY"));
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
    for arguments in [
        &["--unknown"][..],
        &["--help", "extra"][..],
        &["one", "two", "three"][..],
    ] {
        let output = tfm(arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"tfm: unsupported arguments; use --help\n");
    }
}

#[test]
fn two_directory_arguments_validate_the_second_path_without_fallback() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing-second-path");
    let output = Command::new(env!("CARGO_BIN_EXE_tfm"))
        .arg("-2")
        .arg(root.path())
        .arg(&missing)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("missing-second-path")
    );
}

#[test]
fn two_pane_options_preserve_native_locations_and_option_termination() {
    use terminal_file_manager::{Invocation, run};
    for args in [
        vec!["-2"],
        vec!["--two-panes", "one"],
        vec!["one", "two"],
        vec!["-2", "--", "-one", "-two"],
    ] {
        let native = args
            .iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>();
        let Invocation::BrowseOptions(options) = run(&native).unwrap() else {
            panic!("startup options");
        };
        let locations = options.locations();
        if args == ["-2"] {
            assert_eq!(
                locations,
                [std::path::PathBuf::from("."), std::path::PathBuf::from(".")]
            );
        }
        if args.contains(&"-one") {
            assert_eq!(
                locations,
                [
                    std::path::PathBuf::from("-one"),
                    std::path::PathBuf::from("-two")
                ]
            );
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let raw = std::ffi::OsString::from_vec(vec![b'x', 0xff]);
        let Invocation::BrowseOptions(options) = run(&["-2".into(), raw.clone()]).unwrap() else {
            panic!("native path");
        };
        assert_eq!(options.locations()[0].as_os_str(), raw);
        assert_eq!(options.locations()[1].as_os_str(), raw);
    }
}

#[test]
fn explicit_missing_path_reports_failure_without_fallback() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("explicit-missing-directory");
    let output = Command::new(env!("CARGO_BIN_EXE_tfm"))
        .arg(&missing)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("explicit-missing-directory")
    );
    assert!(!missing.exists());
}

#[test]
fn noninteractive_startup_fails_clearly_without_creating_config() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("settings");
    let output = Command::new(env!("CARGO_BIN_EXE_tfm"))
        .current_dir(root.path())
        .env("XDG_CONFIG_HOME", &config)
        .env("APPDATA", &config)
        .env("HOME", root.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("terminal")
    );
    assert!(!config.exists());
}
