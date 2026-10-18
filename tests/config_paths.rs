//! XDG resolution uses isolated executable environments, never the runner's environment.
#![cfg(unix)]
use std::{ffi::OsString, fs, path::Path, process::Command};
use terminal_file_manager::session::SessionStore;

#[test]
fn invalid_xdg_values_use_native_defaults_for_settings_and_session_saves() {
    for value in [None, Some(OsString::new()), Some("relative".into())] {
        check_location(value, None);
    }
    let absolute = tempfile::tempdir().unwrap();
    check_location(Some(absolute.path().into()), Some(absolute.path()));
}

fn check_location(value: Option<OsString>, expected_override: Option<&Path>) {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let cwd = root.path().join("cwd");
    fs::create_dir_all(cwd.join("tfm")).unwrap();
    let base = expected_override
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| default_config(&home));
    let config = base.join("tfm/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    let settings = "two_panes = true\nrestore_session = true\n";
    fs::write(&config, settings).unwrap();
    let sentinel = cwd.join("tfm/config.toml");
    fs::write(&sentinel, "this is invalid TOML").unwrap();
    let mut command = isolated(&home, &cwd, value.as_deref());
    let saved = command.arg("--save-session").arg(&cwd).output().unwrap();
    assert!(saved.status.success(), "{saved:?}");
    let session = SessionStore {
        path: base.join("tfm/session.toml"),
    }
    .load()
    .unwrap();
    assert_eq!(session.locations().unwrap(), [cwd.clone(), cwd.clone()]);
    assert!(session.restore(&Default::default()).unwrap().dual);
    assert_eq!(fs::read_to_string(config).unwrap(), settings);
    assert_eq!(
        fs::read_to_string(sentinel).unwrap(),
        "this is invalid TOML"
    );
    assert!(!cwd.join("tfm/session.toml").exists());
    assert!(!cwd.join("relative").exists());
    let restored = isolated(&home, &cwd, value.as_deref())
        .args(["--restore-session", "--save-session"])
        .output()
        .unwrap();
    assert!(restored.status.success(), "{restored:?}");
}

fn default_config(home: &Path) -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else {
        home.join(".config")
    }
}

fn isolated(home: &Path, cwd: &Path, value: Option<&std::ffi::OsStr>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tfm"));
    command
        .current_dir(cwd)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME");
    if let Some(value) = value {
        command.env("XDG_CONFIG_HOME", value);
    }
    command
}
