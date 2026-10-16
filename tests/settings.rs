//! Settings are optional, validated and written only after a deliberate preference change.
use std::fs;
use terminal_file_manager::settings::{Settings, Store};

#[test]
fn missing_config_does_not_create_files_and_preferences_round_trip() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tfm/config.toml");
    let mut store = Store::load(path.clone()).unwrap();
    assert!(!path.exists());
    store.settings.sidebar = true;
    store.settings.sidebar_left = true;
    store.save().unwrap();
    let restored = Store::load(path).unwrap();
    assert!(restored.settings.sidebar);
    assert!(restored.settings.sidebar_left);
}

#[test]
fn invalid_settings_and_key_conflicts_are_actionable_errors() {
    for text in [
        "mouse = 'yes'",
        "unknown = true",
        "sort = 'typo'",
        "[bindings]\n'Ctrl+p'='quit'\n'ctrl+p'='help'",
    ] {
        assert!(Settings::parse(text).is_err(), "{text}");
    }
    assert!(Settings::parse("[bindings]\n'F12'='toggle-panes'").is_ok());
}

#[test]
fn linked_config_is_rejected_without_modifying_the_target() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    fs::write(&target, "mouse = false\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, root.path().join("config.toml")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&target, root.path().join("config.toml")).unwrap();
    assert!(Store::load(root.path().join("config.toml")).is_err());
    assert_eq!(fs::read_to_string(target).unwrap(), "mouse = false\n");
}
