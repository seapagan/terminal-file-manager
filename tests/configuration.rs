//! Configuration policy uses owned paths; failed writes remain usable and retryable.
use std::fs;
use terminal_file_manager::{
    app::App,
    input::{Action, Bindings, Controller},
    notifications::Severity,
    settings::{Configuration, Settings, Store},
    ui,
};

fn controller(settings: Settings, root: &std::path::Path) -> Controller {
    Controller::new(
        App::new(root.into()),
        Bindings::new(&settings.bindings).unwrap(),
        settings,
    )
}

#[test]
fn missing_configuration_is_created_without_waiting_for_a_dialog_or_preference_change() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("nested/config.toml");
    let mut config = Configuration::load(Some(path.clone())).unwrap();
    let mut c = controller(config.preferences(), root.path());
    assert!(!path.exists());
    config.synchronize(&mut c);
    assert!(
        Store::load(path)
            .unwrap()
            .settings
            .compound_extensions
            .is_none()
    );
    assert!(c.app.notifications.history.is_empty());
}

#[test]
fn automatic_creation_failure_remains_pending_without_any_feature_request() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("parent");
    let path = parent.join("config.toml");
    let mut config = Configuration::load(Some(path.clone())).unwrap();
    let mut c = controller(config.preferences(), root.path());
    fs::write(&parent, "blocked").unwrap();
    config.synchronize(&mut c);
    assert!(!c.preferences_changed && !c.quit);
    assert_eq!(c.app.notifications.history.len(), 1);
    assert!(!path.exists());
    fs::remove_file(parent).unwrap();
    config.synchronize(&mut c);
    assert!(!path.exists());
    let g = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    c.dispatch(Action::Hidden, &g);
    config.synchronize(&mut c);
    assert!(
        Store::load(path)
            .unwrap()
            .settings
            .compound_extensions_checked_version
            .is_none()
    );
}

#[test]
fn startup_save_failure_retains_marker_and_reports_os_error_once_then_recovers() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("parent");
    let path = parent.join("config.toml");
    let mut config = Configuration::load(Some(path.clone())).unwrap();
    let mut c = controller(config.preferences(), root.path());
    fs::write(&parent, "blocked").unwrap();
    c.check_compound_extensions("0.1.0").unwrap();
    config.synchronize(&mut c);
    assert!(!c.preferences_changed);
    assert!(!c.quit);
    assert_eq!(
        c.preferences.compound_extensions_checked_version.as_deref(),
        Some("0.1.0")
    );
    let notice = &c.app.notifications.history[0];
    assert_eq!(notice.severity, Severity::Error);
    assert!(notice.text.contains("Could not save configuration"));
    assert!(notice.text.contains(&path.display().to_string()));
    let diagnostic = Store {
        path: path.clone(),
        settings: c.preferences.clone(),
    }
    .save()
    .unwrap_err();
    assert!(notice.text.contains(&diagnostic));
    config.synchronize(&mut c);
    assert_eq!(c.app.notifications.history.len(), 1);
    assert_eq!(c.app.notifications.history[0].occurrences, 1);
    fs::remove_file(parent).unwrap();
    config.synchronize(&mut c);
    assert!(!path.exists());
    let g = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    c.dispatch(Action::Hidden, &g);
    config.synchronize(&mut c);
    assert!(!c.preferences_changed);
    let saved = Store::load(path).unwrap().settings;
    assert_eq!(
        saved.compound_extensions_checked_version.as_deref(),
        Some("0.1.0")
    );
    assert_eq!(c.app.notifications.history.len(), 1);
}

#[test]
fn interactive_save_failure_keeps_preferences_and_browsing_then_saves_latest_changes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    fs::write(
        &path,
        "mouse = false\ncompound_extensions_checked_version = '9.0.0'\n",
    )
    .unwrap();
    let mut config = Configuration::load(Some(path.clone())).unwrap();
    let mut c = controller(config.preferences(), root.path());
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    let g = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    c.dispatch(Action::Hidden, &g);
    config.synchronize(&mut c);
    assert!(c.preferences.hidden);
    assert!(path.is_dir());
    assert_eq!(c.app.notifications.history[0].occurrences, 1);
    c.app.notifications.dismiss_toasts();
    config.synchronize(&mut c);
    assert!(c.app.notifications.visible().is_empty());
    c.dispatch(Action::ToggleSidebar, &g);
    config.synchronize(&mut c);
    assert!(c.app.sidebar && !c.quit);
    assert_eq!(c.app.notifications.history.len(), 1);
    assert_eq!(c.app.notifications.history[0].occurrences, 2);
    assert_eq!(c.app.notifications.visible(), [0]);
    fs::remove_dir(&path).unwrap();
    config.synchronize(&mut c);
    assert!(!path.exists());
    c.dispatch(Action::Reverse, &g);
    config.synchronize(&mut c);
    let saved = Store::load(path).unwrap().settings;
    assert!(saved.hidden && saved.sidebar && saved.reverse && !saved.mouse);
    assert_eq!(
        saved.compound_extensions_checked_version.as_deref(),
        Some("9.0.0")
    );
}

#[test]
fn invalid_and_nonregular_existing_configuration_is_never_replaced() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    for bytes in [b"unknown = true".as_slice(), b"mouse = false\n\xff"] {
        fs::write(&path, bytes).unwrap();
        let error = Configuration::load(Some(path.clone())).err().unwrap();
        assert!(error.contains(&path.display().to_string()));
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(Configuration::load(Some(path.clone())).is_err());
    assert!(fs::read_dir(path).unwrap().next().is_none());
}

#[cfg(unix)]
#[test]
fn unreadable_configuration_fails_without_fallback_or_overwrite() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    fs::write(&path, "mouse = false").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
    let error = Configuration::load(Some(path.clone())).err().unwrap();
    assert!(error.contains(&path.display().to_string()));
    assert!(error.contains(&fs::read(&path).unwrap_err().to_string()));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), "mouse = false");
}

#[test]
fn unavailable_configuration_location_retains_changes_with_one_general_diagnostic() {
    let root = tempfile::tempdir().unwrap();
    let mut config = Configuration::load(None).unwrap();
    let mut c = controller(config.preferences(), root.path());
    c.check_compound_extensions("0.1.0").unwrap();
    config.synchronize(&mut c);
    config.synchronize(&mut c);
    assert!(!c.preferences_changed && !c.quit);
    assert_eq!(
        config
            .preferences()
            .compound_extensions_checked_version
            .as_deref(),
        Some("0.1.0")
    );
    assert_eq!(c.app.notifications.history.len(), 1);
    assert!(
        c.app.notifications.history[0]
            .text
            .contains("no user config directory")
    );
}
