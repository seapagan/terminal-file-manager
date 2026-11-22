//! Startup persistence and real terminal compatibility review.
use super::*;

#[test]
fn browser_starts_at_cwd_and_explicit_path_and_restores_terminal() {
    let storage = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir_in(storage.path()).unwrap();
    fs::write(root.path().join("browser-sentinel"), "read only").unwrap();
    for explicit in [false, true] {
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
        if explicit {
            command.arg(root.path());
        }
        command.cwd(root.path());
        isolate(&mut command, storage.path());
        let output = drive(
            command,
            &[
                ("browser-sentinel", "\r"),
                ("The file viewer is not available yet.", "\u{1b}p"),
                ("Pane 2", "q"),
            ],
            true,
        );
        assert!(output.contains("browser-sentinel"));
        assert_eq!(
            fs::read(root.path().join("browser-sentinel")).unwrap(),
            b"read only"
        );
        assert_checked(storage.path());
    }
}

#[test]
fn diagnostics_uses_startup_capability_in_a_controlled_real_terminal() {
    let storage = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir_in(storage.path()).unwrap();
    fs::write(root.path().join("diagnostics-sentinel"), "read only").unwrap();
    for (forced, expected, vte) in [
        (None, "TrueColor", None),
        (Some("0"), "NoColor", Some("8400")),
        (Some("1"), "Ansi16", Some("")),
        (Some("2"), "Ansi256", Some("bad")),
        (Some("3"), "TrueColor", Some("8400")),
    ] {
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
        command.cwd(root.path());
        isolate(&mut command, storage.path());
        // Only this child drops inherited overrides; precedence tests stay intact.
        for name in ["NO_COLOR", "FORCE_COLOR", "CLICOLOR", "CLICOLOR_FORCE"] {
            command.env_remove(name);
        }
        command.env("COLORTERM", "truecolor");
        if let Some(value) = vte {
            command.env("VTE_VERSION", value);
        }
        if let Some(value) = forced {
            command.env("FORCE_COLOR", value);
        }
        let scroll = "\u{1b}[6;3~".repeat(200);
        let output = drive(
            command,
            &[
                ("diagnostics-sentinel", "\u{1b}[24~"),
                (&format!("Applied ColorLevel: {expected}"), &scroll),
                // Exact values are checked in the diagnostics buffer tests;
                // differential output may reuse <unset> from the previous row.
                ("VTE_VERSION", "\u{1b}[24~\r"),
                ("The file viewer is not available yet.", "q"),
            ],
            true,
        );
        assert!(output.contains("Startup stdout TTY"));
        assert!(output.contains("xterm-256color"));
    }
    assert_eq!(
        fs::read(root.path().join("diagnostics-sentinel")).unwrap(),
        b"read only"
    );
    assert_checked(storage.path());
}

#[test]
fn startup_compound_review_saves_choices_and_stays_suppressed() {
    use terminal_file_manager::settings::Store;
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("review-ready"), "owned").unwrap();
    let config = root.path().join("config/tfm/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, "mouse = false\nsidebar_left = true\ncompound_extensions = ['.tar.gz']\ncompound_extensions_extend = ['.tar.gz', '.user.js']").unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
    command.cwd(root.path());
    isolate(&mut command, root.path());
    drive(
        command,
        &[
            ("Compound extension compatibility", "d"),
            ("Review custom extensions", "r"),
            ("review-ready", "q"),
        ],
        true,
    );
    let saved = Store::load(config.clone()).unwrap().settings;
    assert!(saved.compound_extensions.is_none());
    assert_eq!(saved.compound_extensions_extend, [".user.js"]);
    assert!(saved.sidebar_left && !saved.mouse);
    assert_eq!(
        saved.compound_extensions_checked_version.as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
    command.cwd(root.path());
    isolate(&mut command, root.path());
    let output = drive(command, &[("review-ready", "q")], true);
    assert!(!output.contains("Compound extension compatibility"));
    assert!(
        Store::load(config)
            .unwrap()
            .settings
            .compound_extensions
            .is_none()
    );
}

fn assert_checked(root: &std::path::Path) {
    let store =
        terminal_file_manager::settings::Store::load(root.join("config/tfm/config.toml")).unwrap();
    assert_eq!(
        store
            .settings
            .compound_extensions_checked_version
            .as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
}
