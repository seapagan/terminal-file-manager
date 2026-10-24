//! Local mutations through the executable and native PTY/ConPTY.
mod terminal_driver;
use portable_pty::CommandBuilder;
use std::{fs, path::Path};
use terminal_driver::drive;

fn command(root: &Path, left: &Path, right: Option<&Path>) -> CommandBuilder {
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tfm"));
    if let Some(right) = right {
        command.args(["-2".as_ref(), left.as_os_str(), right.as_os_str()]);
    } else {
        command.arg(left);
    }
    command.cwd(root);
    for variable in ["HOME", "USERPROFILE"] {
        command.env(variable, root);
    }
    for (variable, directory) in [
        ("XDG_CONFIG_HOME", "config"),
        ("APPDATA", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        command.env(variable, root.join(directory));
    }
    command.env("GIT_CONFIG_GLOBAL", root.join("gitconfig"));
    command.env("GIT_CONFIG_NOSYSTEM", "1");
    command.env("NO_COLOR", "1");
    command.env("TERM", "xterm-256color");
    command.env_remove("VTE_VERSION");
    command
}

#[test]
fn mkdir_runs_from_the_real_dialog_and_restores_the_terminal() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("sentinel"), "preserved").unwrap();
    drive(
        command(root.path(), root.path(), None),
        &[
            ("sentinel", "\x1b[18~"),
            ("Create directory", "created-directory\r"),
            ("1 directory created", "q"),
        ],
        true,
    );
    assert!(root.path().join("created-directory").is_dir());
    assert_eq!(
        fs::read(root.path().join("sentinel")).unwrap(),
        b"preserved"
    );
}

#[test]
fn copy_and_move_replace_are_accessible_from_the_real_dual_pane_manager() {
    let root = tempfile::tempdir().unwrap();
    let left = root.path().join("left");
    let right = root.path().join("right");
    fs::create_dir(&left).unwrap();
    fs::create_dir(&right).unwrap();
    fs::write(left.join("payload"), "incoming").unwrap();
    fs::write(left.join("zzz-left-ready"), "left sentinel").unwrap();
    fs::write(right.join("aaa-right-ready"), "right sentinel").unwrap();
    drive(
        command(root.path(), &left, Some(&right)),
        &[
            ("zzz-left-ready", "\t"),
            ("aaa-right-ready", "\x1b[Z\x1b[15~"),
            ("1 file copied", "q"),
        ],
        false,
    );
    assert_eq!(fs::read(right.join("payload")).unwrap(), b"incoming");
    fs::write(right.join("payload"), "previous").unwrap();
    drive(
        command(root.path(), &left, Some(&right)),
        &[
            ("zzz-left-ready", "\t"),
            ("aaa-right-ready", "\x1b[Z\x1b[17~"),
            ("Destination conflict", "r"),
            ("1 file moved", "q"),
        ],
        false,
    );
    assert!(!left.join("payload").exists());
    assert_eq!(fs::read(right.join("payload")).unwrap(), b"incoming");
}

#[test]
fn permanent_delete_requires_separate_confirmation_and_enter_cancels() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("selected"), "owned").unwrap();
    drive(
        command(root.path(), root.path(), None),
        &[("selected", "\x1bd"), ("Preview: 1 file", "\rq")],
        false,
    );
    assert_eq!(fs::read(root.path().join("selected")).unwrap(), b"owned");
    drive(
        command(root.path(), root.path(), None),
        &[
            ("selected", "\x1bd"),
            ("Preview: 1 file", "D"),
            ("1 file permanently deleted", "q"),
        ],
        false,
    );
    assert!(!root.path().join("selected").exists());
}
