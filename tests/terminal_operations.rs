//! Local mutations through the executable and native PTY/ConPTY.
mod terminal_driver;
use portable_pty::CommandBuilder;
use std::{fs, path::Path};
use terminal_driver::drive;

#[test]
fn platform_refresh_renders_external_changes_and_terminal_reentry_is_clean() {
    use std::time::{Duration, Instant};
    use terminal_driver::{Script, Session};
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("initial-ready"), "owned").unwrap();
    for attempt in 0..2 {
        let mut session = Session::open(command(root.path(), root.path(), None)).unwrap();
        let mut transcript = Vec::new();
        let mut script = Script::default();
        let deadline = Instant::now() + Duration::from_secs(20);
        while script.step == 0 {
            assert!(Instant::now() < deadline, "initial listing timed out");
            session
                .read_step(
                    Duration::from_millis(20),
                    &mut script,
                    &[("initial-ready", "")],
                    &mut transcript,
                    false,
                )
                .unwrap();
        }
        let marker = format!("external-arrival-{attempt}");
        let changed = Instant::now();
        fs::write(root.path().join(&marker), "external").unwrap();
        #[cfg(windows)]
        manual_refresh_after_idle(&mut session, &mut script, &mut transcript, &marker);
        session
            .drive(&[(&marker, "q")], deadline, &mut transcript, false)
            .unwrap();
        eprintln!(
            "Native PTY external create, render and clean exit: {:?}",
            changed.elapsed()
        );
        assert!(transcript.windows(8).any(|w| w == b"\x1b[?2004l"));
        assert!(transcript.windows(8).any(|w| w == b"\x1b[?1049l"));
    }
}

#[cfg(windows)]
fn manual_refresh_after_idle(
    session: &mut terminal_driver::Session,
    script: &mut terminal_driver::Script,
    transcript: &mut Vec<u8>,
    marker: &str,
) {
    use std::{
        io::Write,
        time::{Duration, Instant},
    };
    // Detect regression to the obsolete two-second Windows polling policy.
    const MANUAL_ONLY_IDLE_WINDOW: Duration = Duration::from_millis(2200);
    let idle = Instant::now();
    while idle.elapsed() < MANUAL_ONLY_IDLE_WINDOW {
        session
            .read_step(Duration::from_millis(20), script, &[], transcript, false)
            .unwrap();
    }
    assert!(!String::from_utf8_lossy(transcript).contains(marker));
    let input = session.writer.as_mut().unwrap();
    input.write_all(b"r").unwrap();
    input.flush().unwrap();
}

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
        &[("selected", "\x1bd"), ("1 file · 5 B", "\rq")],
        false,
    );
    assert_eq!(fs::read(root.path().join("selected")).unwrap(), b"owned");
    drive(
        command(root.path(), root.path(), None),
        &[
            ("selected", "\x1bd"),
            ("1 file · 5 B", "D"),
            ("1 file permanently deleted", "q"),
        ],
        false,
    );
    assert!(!root.path().join("selected").exists());
}

#[test]
fn completed_directory_delete_can_be_followed_by_a_real_create_dialog() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("selected-directory")).unwrap();
    fs::write(root.path().join("selected-directory/child"), "owned").unwrap();
    fs::write(root.path().join("zzz-ready"), "retained").unwrap();
    drive(
        command(root.path(), root.path(), None),
        &[
            ("zzz-ready", "\x1bd"),
            ("1 file, 1 directory · 5 B", "D"),
            ("permanently deleted", "\x1b[18~"),
            ("Directory name:", "after-delete\r"),
            ("after-delete/", "q"),
        ],
        false,
    );
    assert!(!root.path().join("selected-directory").exists());
    assert!(root.path().join("after-delete").is_dir());
    assert_eq!(
        fs::read(root.path().join("zzz-ready")).unwrap(),
        b"retained"
    );
}
