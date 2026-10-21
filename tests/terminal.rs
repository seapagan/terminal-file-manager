//! Native PTY/ConPTY lifecycle and real executable acceptance.
use portable_pty::CommandBuilder;
mod terminal_driver;
use std::fs;
use terminal_driver::drive;

#[test]
fn browser_starts_at_cwd_and_explicit_path_and_restores_terminal() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("browser-sentinel"), "read only").unwrap();
    for explicit in [false, true] {
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
        if explicit {
            command.arg(root.path());
        }
        command.cwd(root.path());
        isolate(&mut command, root.path());
        let output = drive(
            command,
            &[
                ("browser-sentinel", "\r"),
                ("Viewer unavailable until M6", "\u{1b}p"),
                ("Pane 2", "q"),
            ],
            true,
        );
        assert!(output.contains("browser-sentinel"));
        assert_eq!(
            fs::read(root.path().join("browser-sentinel")).unwrap(),
            b"read only"
        );
        assert!(!root.path().join("config/tfm/config.toml").exists());
    }
}

#[test]
fn preference_change_survives_exit_from_other_pane_and_real_restart() {
    use terminal_file_manager::{listing::Sort, settings::Store};
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("preference-sentinel"), "read only").unwrap();
    let config = root.path().join("config/tfm/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        "mouse = false\nsidebar_left = true\n[bindings]\nF12 = 'quit'\n",
    )
    .unwrap();
    for step in [("preference-sentinel", "s\u{1b}p\tq"), ("Name", "q")] {
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
        command.cwd(root.path());
        isolate(&mut command, root.path());
        drive(command, &[step], false);
        let saved = Store::load(config.clone()).unwrap().settings;
        assert_eq!(saved.sort, Sort::Name);
        assert!(!saved.mouse);
        assert!(saved.sidebar_left);
        assert_eq!(
            saved.bindings.get("F12"),
            Some(&terminal_file_manager::input::Action::Quit)
        );
    }
}

#[test]
fn two_explicit_locations_and_configured_dual_startup_run_in_a_real_terminal() {
    let root = tempfile::tempdir().unwrap();
    let left = root.path().join("left");
    let right = root.path().join("right");
    fs::create_dir(&left).unwrap();
    fs::create_dir(&right).unwrap();
    fs::write(left.join("left-startup"), "one").unwrap();
    fs::write(right.join("right-startup"), "two").unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
    command.arg("-2");
    command.arg(&left);
    command.arg(&right);
    command.cwd(root.path());
    isolate(&mut command, root.path());
    let output = drive(
        command,
        &[("left-startup", "\t"), ("right-startup", "q")],
        false,
    );
    assert!(output.contains("left-startup"));
    assert_eq!(fs::read(right.join("right-startup")).unwrap(), b"two");
    let config = root.path().join("config/tfm/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, "two_panes=true\nmouse=false").unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
    command.arg(&left);
    command.cwd(root.path());
    isolate(&mut command, root.path());
    let output = drive(command, &[("left-startup", "\t"), ("Pane 2", "q")], false);
    assert!(output.contains("left"));
}

#[test]
fn terminal_restores_after_returned_error_and_unwind() {
    let root = tempfile::tempdir().unwrap();
    let mut modes = vec!["error", "panic", "incident"];
    if cfg!(windows) {
        modes.push("keys");
    }
    for mode in modes {
        // Relaunches this test child without a security decision; reassess if privileged.
        // nosemgrep: rust.lang.security.current-exe.current-exe
        let mut command = CommandBuilder::new(std::env::current_exe().unwrap());
        command.args(["--ignored", "--exact", "terminal_child", "--nocapture"]);
        command.env("TFM_TEST_TERMINAL_MODE", mode);
        isolate(&mut command, root.path());
        let owned = console_steps(mode);
        let steps: Vec<_> = owned
            .iter()
            .map(|(label, input)| (*label, input.as_str()))
            .collect();
        let output = drive(command, &steps, true);
        assert!(output.contains("RESTORATION-CHECK-PASSED"));
    }
}

#[test]
#[ignore = "Owned PTY child entry point; exercised by terminal_restores_after_returned_error_and_unwind"]
fn terminal_child() {
    let mode = std::env::var("TFM_TEST_TERMINAL_MODE").unwrap();
    #[cfg(windows)]
    if mode == "keys" {
        windows_console_probe();
        return;
    }
    let result = std::panic::catch_unwind(|| {
        terminal_file_manager::terminal::with_terminal(true, |_| -> std::io::Result<()> {
            if mode == "panic" {
                panic!("owned terminal unwind test");
            }
            if mode == "incident" {
                incident_exit_probe();
                return Ok(());
            }
            Err(std::io::Error::other("owned terminal error test"))
        })
    });
    if mode == "panic" {
        assert!(result.is_err());
    } else if mode == "incident" {
        assert!(result.unwrap().is_ok());
    } else {
        assert!(result.unwrap().is_err());
    }
    assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
    println!("RESTORATION-CHECK-PASSED");
}

// A controlled worker record exercises exit/cleanup without provoking real data loss.
fn incident_controller() -> (tempfile::TempDir, terminal_file_manager::input::Controller) {
    use terminal_file_manager::{
        app::App,
        input::{Bindings, Controller},
        operations::{Operation, Outcome, Plan, Update},
    };
    let root = tempfile::tempdir().unwrap();
    let plan = Plan::mkdir(root.path().join("owned-directory"));
    let mut record = terminal_file_manager::operations::execute(
        &plan,
        Default::default(),
        &false.into(),
        |_| terminal_file_manager::operations::Decision::Skip,
    );
    record.entries[0].outcome = Outcome::SafetyIncident;
    let mut plan = plan;
    plan.operation = Operation::Trash;
    let app = App::new(root.path().to_owned());
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    controller.register_operation(1, plan, Default::default(), None);
    controller.complete_operation(Update::Finished { id: 1, record });
    (root, controller)
}

fn incident_exit_probe() {
    let (_root, mut controller) = incident_controller();
    let geometry = terminal_file_manager::ui::geometry(
        &controller.app,
        ratatui::layout::Rect::new(0, 0, 100, 30),
    );
    controller.dispatch(terminal_file_manager::input::Action::Quit, &geometry);
    for code in "ACKNOWLEDGE"
        .chars()
        .map(crossterm::event::KeyCode::Char)
        .chain([crossterm::event::KeyCode::F(10)])
    {
        controller.handle(
            crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
                code,
                crossterm::event::KeyModifiers::NONE,
            )),
            &geometry,
            std::time::Duration::ZERO,
        );
    }
    assert!(controller.quit);
    assert!(
        controller.app.operations.history[0]
            .record
            .as_ref()
            .unwrap()
            .incident()
    );
}

// ConPTY requests Win32 input mode, not Kitty CSI-u. Keep the unsupported probes.
const CONSOLE_PROBES: &[(&str, &str)] = &[
    ("PROBE-WIN32-QUESTION", "\u{1b}[0;0;63;1;16;1_"),
    ("PROBE-WIN32-SECTION", "\u{1b}[0;0;167;1;16;1_"),
    ("PROBE-WIN32-LETTER", "\u{1b}[0;0;88;1;16;1_"),
    ("PROBE-UNSUPPORTED-CSI-QUESTION", "\u{1b}[63;2u"),
    ("PROBE-UNSUPPORTED-CSI-ALTERNATE", "\u{1b}[49:167;2u"),
    ("PROBE-UNSUPPORTED-CSI-LETTER", "\u{1b}[120;2u"),
    ("PROBE-RAW-SECTION", "§"),
];

fn console_steps(mode: &str) -> Vec<(&'static str, String)> {
    if mode != "keys" {
        return Vec::new();
    }
    CONSOLE_PROBES
        .iter()
        .map(|(label, input)| {
            // F24 is an ordered input barrier, so unsupported input needs no sleep.
            (*label, format!("{input}\u{1b}[135;0;0;1;0;1_"))
        })
        .collect()
}

#[cfg(windows)]
fn windows_console_probe() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use terminal_file_manager::input::{Action, Bindings};
    terminal_file_manager::terminal::with_terminal(false, |_| {
        assert!(!crossterm::terminal::supports_keyboard_enhancement()?);
        let bindings = Bindings::new(&std::collections::BTreeMap::from([
            ("?".into(), Action::Quit),
            ("§".into(), Action::Quit),
            ("Shift+x".into(), Action::Quit),
        ]))
        .unwrap();
        for (index, (label, _)) in CONSOLE_PROBES.iter().enumerate() {
            println!("{label}");
            let keys = read_console_probe()?;
            println!("EVENTS-{label}: {keys:?}");
            if index < 3 {
                assert_eq!(keys.len(), 1, "{label}: {keys:?}");
                assert_eq!(keys[0].code, KeyCode::Char(['?', '§', 'X'][index]));
                assert_eq!(keys[0].modifiers, KeyModifiers::SHIFT);
                assert_eq!(bindings.action(keys[0]), Some(Action::Quit));
            } else if *label == "PROBE-RAW-SECTION" {
                assert_raw_section_dispatch(&keys);
            } else {
                assert!(
                    keys.iter()
                        .all(|key| bindings.action(*key) != Some(Action::Quit)),
                    "{label}: {keys:?}"
                );
            }
        }
        Ok(())
    })
    .unwrap();
    assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
    println!("RESTORATION-CHECK-PASSED");
}

#[cfg(windows)]
fn assert_raw_section_dispatch(keys: &[crossterm::event::KeyEvent]) {
    use crossterm::event::{Event, KeyCode, KeyEventKind};
    use terminal_file_manager::{
        app::App,
        input::{Action, Bindings, Controller},
        ui,
    };
    assert!(
        !keys.is_empty(),
        "ConPTY did not deliver the raw section sign"
    );
    for key in keys {
        assert_eq!(key.code, KeyCode::Char('§'));
        let bindings = Bindings::new(&std::collections::BTreeMap::from([(
            "§".into(),
            Action::Quit,
        )]))
        .unwrap();
        let expected =
            key.kind == KeyEventKind::Press && bindings.action(*key) == Some(Action::Quit);
        let mut controller = Controller::new(
            App::new("owned-native-probe".into()),
            bindings,
            Default::default(),
        );
        let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
        controller.handle(Event::Key(*key), &geometry, std::time::Duration::ZERO);
        assert_eq!(controller.quit, expected, "{key:?}");
    }
}

#[cfg(windows)]
fn read_console_probe() -> std::io::Result<Vec<crossterm::event::KeyEvent>> {
    use crossterm::event::{Event, KeyCode, KeyEventKind};
    let mut keys = Vec::new();
    loop {
        if let Event::Key(key) = crossterm::event::read()? {
            if key.code == KeyCode::F(24) && key.kind == KeyEventKind::Press {
                return Ok(keys);
            }
            assert!(keys.len() < 64, "input barrier missing: {keys:?}");
            keys.push(key);
        }
    }
}

fn isolate(command: &mut CommandBuilder, root: &std::path::Path) {
    command.env("HOME", root);
    command.env("USERPROFILE", root);
    command.env("XDG_CONFIG_HOME", root.join("config"));
    command.env("XDG_DATA_HOME", root.join("data"));
    command.env("XDG_CACHE_HOME", root.join("cache"));
    command.env("APPDATA", root.join("config"));
    command.env("GIT_CONFIG_GLOBAL", root.join("gitconfig"));
    command.env("GIT_CONFIG_NOSYSTEM", "1");
    command.env("NO_COLOR", "1");
    command.env("TERM", "xterm-256color");
    command.env_remove("VTE_VERSION");
}

#[test]
fn printable_bindings_match_legacy_and_enhanced_layout_characters() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("keyboard-sentinel"), "read only").unwrap();
    let config = root.path().join("config/tfm/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        "[bindings]\n'?'='quit'\n'§'='quit'\n'Shift+x'='quit'\n",
    )
    .unwrap();
    for encoded in printable_inputs() {
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
        command.cwd(root.path());
        isolate(&mut command, root.path());
        drive(command, &[("keyboard-sentinel", encoded)], false);
    }
    assert_eq!(
        fs::read(root.path().join("keyboard-sentinel")).unwrap(),
        b"read only"
    );
}

fn printable_inputs() -> Vec<&'static str> {
    if cfg!(windows) {
        vec![
            "?",
            // Raw § can become an Alt-code release on ConPTY's active layout.
            // Express the intended unmodified press through its requested protocol.
            "\u{1b}[0;0;167;1;0;1_",
            "X",
            CONSOLE_PROBES[0].1,
            CONSOLE_PROBES[1].1,
            CONSOLE_PROBES[2].1,
        ]
    } else {
        vec![
            "?",
            "\u{1b}[63;2u",
            "\u{1b}[49:167;2u",
            "§",
            "X",
            "\u{1b}[120;2u",
        ]
    }
}

#[test]
fn diagnostics_uses_startup_capability_in_a_controlled_real_terminal() {
    let root = tempfile::tempdir().unwrap();
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
        isolate(&mut command, root.path());
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
                ("Viewer unavailable until M6", "q"),
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
    assert!(!root.path().join("config/tfm/config.toml").exists());
}

#[test]
fn shifted_arrow_marks_a_range_in_a_real_terminal() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("range-a"), "one").unwrap();
    fs::write(root.path().join("range-b"), "two").unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
    command.cwd(root.path());
    isolate(&mut command, root.path());
    drive(
        command,
        &[("range-a", "\u{1b}[1;2B"), ("Selected: 2 visible", "q")],
        false,
    );
    assert_eq!(fs::read(root.path().join("range-a")).unwrap(), b"one");
}

#[test]
fn save_quit_restores_both_navigated_locations_and_normal_quit_keeps_session() {
    use terminal_file_manager::session::SessionStore;
    let root = tempfile::tempdir().unwrap();
    let left = root.path().join("left");
    let right = root.path().join("right");
    fs::create_dir_all(left.join("child")).unwrap();
    fs::create_dir_all(&right).unwrap();
    fs::write(left.join("child/left-session"), "one").unwrap();
    fs::write(right.join("right-session"), "two").unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
    command.arg("-2");
    command.arg(&left);
    command.arg(&right);
    command.cwd(root.path());
    isolate(&mut command, root.path());
    drive(
        command,
        &[("child", "\r"), ("left-session", "\t"), ("Pane 2", "Q")],
        false,
    );
    let store = SessionStore {
        path: root.path().join("config/tfm/session.toml"),
    };
    assert_eq!(
        store.load().unwrap().locations().unwrap(),
        [left.join("child"), right]
    );
    let saved = fs::read(&store.path).unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
    command.arg("--restore-session");
    command.cwd(root.path());
    isolate(&mut command, root.path());
    let output = drive(command, &[("left-session", "\t"), ("Pane 1", "q")], false);
    assert!(output.contains("right-session"));
    assert_eq!(fs::read(&store.path).unwrap(), saved);
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
    command.arg(root.path());
    command.cwd(root.path());
    isolate(&mut command, root.path());
    drive(command, &[("left", "q")], false);
    assert_eq!(fs::read(&store.path).unwrap(), saved);
}
