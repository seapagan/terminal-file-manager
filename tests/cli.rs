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
    let output = isolated_session_command(root.path())
        .arg("-2")
        .arg(root.path())
        .arg(&missing)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains(&missing.display().to_string())
    );
}

#[test]
fn two_pane_options_preserve_native_locations_and_option_termination() {
    use terminal_file_manager::{Invocation, run};
    for (args, expected) in [
        (vec!["-2"], [".", "."]),
        (vec!["--two-panes", "one"], ["one", "one"]),
        (vec!["one", "two"], ["one", "two"]),
        (vec!["-2", "--", "-one", "-two"], ["-one", "-two"]),
    ] {
        let native = args
            .iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>();
        let Invocation::BrowseOptions(options) = run(&native).unwrap() else {
            panic!("startup options");
        };
        assert_eq!(options.locations(), expected.map(std::path::PathBuf::from));
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
    let output = isolated_session_command(root.path())
        .arg(&missing)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains(&missing.display().to_string())
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
    assert_eq!(
        output.stderr,
        b"tfm: interactive browsing requires a terminal on stdin and stdout\n"
    );
    assert!(!config.exists());
}

fn isolated_session_command(root: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tfm"));
    command
        .current_dir(root)
        .env("HOME", root)
        .env("USERPROFILE", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("APPDATA", root.join("config"));
    command
}

#[test]
fn explicit_headless_session_save_and_restore_failures_preserve_intent() {
    use terminal_file_manager::session::SessionStore;
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("saved-location");
    std::fs::create_dir(&directory).unwrap();
    let output = isolated_session_command(root.path())
        .arg("--save-session")
        .arg("-2")
        .arg(&directory)
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    let store = SessionStore {
        path: root.path().join("config/tfm/session.toml"),
    };
    assert_eq!(
        store.load().unwrap().locations().unwrap(),
        [directory.clone(), directory.clone()]
    );
    let saved = std::fs::read(&store.path).unwrap();
    std::fs::remove_dir(&directory).unwrap();
    let output = isolated_session_command(root.path())
        .arg("--restore-session")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("saved-location")
    );
    assert_eq!(std::fs::read(&store.path).unwrap(), saved);
}

#[test]
fn explicit_paths_bypass_configured_restore_and_conflicting_cli_restore_fails() {
    use terminal_file_manager::session::SessionStore;
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("config/tfm")).unwrap();
    let store = SessionStore {
        path: root.path().join("config/tfm/session.toml"),
    };
    std::fs::write(
        root.path().join("config/tfm/config.toml"),
        "restore_session=true",
    )
    .unwrap();
    let output = isolated_session_command(root.path())
        .arg("explicit-missing")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("explicit-missing")
    );
    let output = isolated_session_command(root.path())
        .arg("--save-session")
        .arg(root.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        store.load().unwrap().locations().unwrap(),
        [root.path().to_owned(), root.path().to_owned()]
    );
    let output = isolated_session_command(root.path())
        .args(["--restore-session", "."])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot be combined")
    );
}

#[test]
fn configured_restore_uses_saved_locations_instead_of_cwd() {
    use terminal_file_manager::session::SessionStore;
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("remembered");
    std::fs::create_dir(&directory).unwrap();
    assert!(
        isolated_session_command(root.path())
            .arg("--save-session")
            .arg(&directory)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(
        root.path().join("config/tfm/config.toml"),
        "restore_session=true",
    )
    .unwrap();
    assert!(
        isolated_session_command(root.path())
            .arg("--save-session")
            .status()
            .unwrap()
            .success()
    );
    let store = SessionStore {
        path: root.path().join("config/tfm/session.toml"),
    };
    assert_eq!(
        store.load().unwrap().locations().unwrap(),
        [directory.clone(), directory]
    );
}
