//! Smoke tests for the implemented executable boundary.

use std::process::{Command, Output};

fn tfm(arguments: &[&str]) -> Output {
    let root = tempfile::tempdir().unwrap();
    isolated_session_command(root.path())
        .args(arguments)
        .output()
        .expect("tfm must execute")
}

#[path = "cli/help.rs"]
mod help;

#[test]
fn version_uses_the_executable_name_and_manifest_version() {
    let expected = format!(
        "tfm {}\nCopyright © 2026 Grant Ramsay\nLicensed under {}\n",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_LICENSE")
    );
    for argument in ["--version", "-V"] {
        assert_eq!(version_output(argument, &[]), expected);
    }
}

#[test]
fn version_colours_only_the_name_and_licence_identifiers() {
    let sgr = regex::Regex::new("\u{1b}\\[[0-9;]*m").unwrap();
    let licence = env!("CARGO_PKG_LICENSE")
        .split_whitespace()
        .map(|token| match token {
            "OR" | "AND" => token.to_owned(),
            _ => format!("\u{1b}[32m{token}\u{1b}[0m"),
        })
        .collect::<Vec<_>>()
        .join(" ");
    for argument in ["--version", "-V"] {
        let plain = version_output(argument, &[("NO_COLOR", "1"), ("FORCE_COLOR", "1")]);
        assert!(!plain.contains('\u{1b}'));
        let coloured = version_output(argument, &[("FORCE_COLOR", "1")]);
        assert_eq!(
            coloured,
            format!(
                "\u{1b}[1;36mtfm\u{1b}[0m {}\nCopyright © 2026 Grant Ramsay\nLicensed under {licence}\n",
                env!("CARGO_PKG_VERSION")
            )
        );
        assert_eq!(sgr.replace_all(&coloured, ""), plain);
    }
}

fn version_output(argument: &str, policy: &[(&str, &str)]) -> String {
    let root = tempfile::tempdir().unwrap();
    let mut command = isolated_session_command(root.path());
    let output = command
        .arg(argument)
        .envs(policy.iter().copied())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
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
    let output = isolated_session_command(root.path())
        .env("XDG_CONFIG_HOME", &config)
        .env("APPDATA", &config)
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
    for key in [
        "NO_COLOR",
        "FORCE_COLOR",
        "CLICOLOR_FORCE",
        "CLICOLOR",
        "TERM",
        "COLORTERM",
    ] {
        command.env_remove(key);
    }
    command
        .current_dir(root)
        .env("HOME", root)
        .env("USERPROFILE", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("APPDATA", root.join("config"));
    command
}

#[test]
fn malformed_config_errors_preserve_diagnostic_lines() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config/tfm/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "restore_session =\n").unwrap();
    let output = isolated_session_command(root.path()).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("tfm: "));
    assert!(stderr.contains("TOML parse error"), "{stderr}");
    assert!(stderr.contains("\n1 | restore_session =\n"), "{stderr}");
    assert!(stderr.contains("\n  |"), "{stderr}");
}

#[test]
fn cli_diagnostic_colours_distinguish_structure_from_source_and_explanation() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config").join("tfm").join("config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "restore_session =\n").unwrap();
    let stderr = coloured_config_error(root.path(), &[("FORCE_COLOR", "1")]);
    assert!(
        stderr.starts_with("\u{1b}[1;31mtfm:\u{1b}[0m "),
        "{stderr:?}"
    );
    assert!(
        stderr.contains(&format!("\u{1b}[36m{}\u{1b}[0m: ", config.display())),
        "{stderr:?}"
    );
    assert!(stderr.contains("\u{1b}[1;31mTOML parse error at line 1, column 18\u{1b}[0m"));
    assert!(stderr.contains("\u{1b}[2m1 |\u{1b}[0m restore_session =\n"));
    assert!(stderr.contains("\u{1b}[2m  |\u{1b}[0m"));
    assert!(stderr.contains("\u{1b}[91m                  ^\u{1b}[0m"));
    assert!(
        stderr
            .contains("\u{1b}[33mstring values must be quoted\u{1b}[0m, expected literal string\n")
    );
}

#[test]
fn cli_diagnostics_follow_library_colour_policy_without_changing_content() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config").join("tfm").join("config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "restore_session =\n").unwrap();
    let plain = coloured_config_error(root.path(), &[]);
    assert!(!plain.contains('\u{1b}'));
    for policy in [
        vec![("NO_COLOR", "1"), ("FORCE_COLOR", "1")],
        vec![("FORCE_COLOR", "0")],
        vec![("CLICOLOR", "0")],
        vec![("TERM", "dumb")],
    ] {
        assert_eq!(coloured_config_error(root.path(), &policy), plain);
    }
    let forced = coloured_config_error(root.path(), &[("FORCE_COLOR", "1")]);
    let sgr = regex::Regex::new("\u{1b}\\[[0-9;]*m").unwrap();
    assert_eq!(sgr.replace_all(&forced, ""), plain);
}

#[test]
fn cli_diagnostics_colour_only_explicit_valid_option_lists_green() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config").join("tfm").join("config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let sgr = regex::Regex::new("\u{1b}\\[[0-9;]*m").unwrap();
    for (setting, options) in [
        ("metadata_presentation", "`sidebar` or `popup`"),
        ("grouping", "`first`, `last`, `mixed`"),
    ] {
        std::fs::write(&config, format!("{setting} = 'invalid'\n")).unwrap();
        let plain = coloured_config_error(root.path(), &[("NO_COLOR", "1")]);
        let forced = coloured_config_error(root.path(), &[("FORCE_COLOR", "1")]);
        assert!(
            forced.contains(&format!("\u{1b}[32m{options}\u{1b}[0m")),
            "{forced:?}"
        );
        assert_eq!(sgr.replace_all(&forced, ""), plain);
        assert!(!plain.contains('\u{1b}'));
    }
}

fn coloured_config_error(root: &std::path::Path, policy: &[(&str, &str)]) -> String {
    let mut command = isolated_session_command(root);
    let output = command.envs(policy.iter().copied()).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    String::from_utf8(output.stderr).unwrap()
}

#[cfg(unix)]
#[test]
fn cli_errors_preserve_line_breaks_and_escape_unsafe_characters() {
    let root = tempfile::tempdir().unwrap();
    let name = "missing\n\nλ界\\n\r\n\t\u{7}\u{1b}[31m\u{85}\u{202e}\u{2069}\u{2028}\u{2029}\n";
    let output = isolated_session_command(root.path())
        .arg(name)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("tfm: "));
    assert!(
        stderr.contains("missing\n\nλ界\\n\\r\n\\t\\u{7}\\u{1b}[31m\\u{85}\\u{202e}\\u{2069}\\u{2028}\\u{2029}\n"),
        "{stderr}"
    );
    assert!(stderr.chars().all(|c| !c.is_control() || c == '\n'));
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

#[test]
fn single_paths_and_terminated_options_keep_their_invocation_variants() {
    use terminal_file_manager::{Invocation, run};
    for path in ["", ".", "native-界"] {
        let Invocation::Browse(actual) = run(&[path.into()]).unwrap() else {
            panic!("a single plain path keeps Browse intent");
        };
        assert_eq!(actual, std::path::PathBuf::from(path));
    }
    for path in ["--help", "--version", "--restore-session", "--"] {
        let Invocation::Browse(actual) = run(&["--".into(), path.into()]).unwrap() else {
            panic!("a terminated path cannot become an option");
        };
        assert_eq!(actual, std::path::PathBuf::from(path));
    }
}

#[test]
fn startup_option_termination_keeps_flags_and_validation_order() {
    use terminal_file_manager::{Invocation, run};
    let arguments =
        ["--save-session", "--", "--restore-session", "--two-panes"].map(std::ffi::OsString::from);
    let Invocation::BrowseOptions(options) = run(&arguments).unwrap() else {
        panic!("startup options");
    };
    assert!(options.save_session);
    assert!(!options.restore_session);
    assert!(!options.two_panes);
    assert_eq!(
        options.locations(),
        ["--restore-session", "--two-panes"].map(std::path::PathBuf::from)
    );
    for arguments in [
        vec!["--restore-session", "one", "two", "three"],
        vec!["--restore-session", "one", "--unknown"],
    ] {
        let native = arguments
            .iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>();
        assert_eq!(
            run(&native).unwrap_err(),
            "unsupported arguments; use --help"
        );
    }
    let native = ["--restore-session", "one"].map(std::ffi::OsString::from);
    assert_eq!(
        run(&native).unwrap_err(),
        "--restore-session cannot be combined with explicit paths"
    );
}
