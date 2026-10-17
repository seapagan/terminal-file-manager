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
    for mode in ["error", "panic"] {
        // Relaunches this test child without a security decision; reassess if privileged.
        // nosemgrep: rust.lang.security.current-exe.current-exe
        let mut command = CommandBuilder::new(std::env::current_exe().unwrap());
        command.args(["--ignored", "--exact", "terminal_child", "--nocapture"]);
        command.env("TFM_TEST_TERMINAL_MODE", mode);
        isolate(&mut command, root.path());
        let output = drive(command, &[], true);
        assert!(output.contains("RESTORATION-CHECK-PASSED"));
    }
}

#[test]
#[ignore = "Owned PTY child entry point; exercised by terminal_restores_after_returned_error_and_unwind"]
fn terminal_child() {
    let mode = std::env::var("TFM_TEST_TERMINAL_MODE").unwrap();
    let result = std::panic::catch_unwind(|| {
        terminal_file_manager::terminal::with_terminal(true, |_| -> std::io::Result<()> {
            if mode == "panic" {
                panic!("owned terminal unwind test");
            }
            Err(std::io::Error::other("owned terminal error test"))
        })
    });
    if mode == "panic" {
        assert!(result.is_err());
    } else {
        assert!(result.unwrap().is_err());
    }
    assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
    println!("RESTORATION-CHECK-PASSED");
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
