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
                ("Viewer unavailable until M6", "\u{10}"),
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
fn terminal_restores_after_returned_error_and_unwind() {
    let root = tempfile::tempdir().unwrap();
    for mode in ["error", "panic"] {
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
