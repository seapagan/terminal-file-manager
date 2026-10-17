//! Settings are optional, validated and written only after a deliberate preference change.
use std::fs;
use terminal_file_manager::settings::{Settings, Store};
use terminal_file_manager::{
    app::App,
    app::IgnoreMode,
    input::{Action, Bindings, Controller},
    listing::{Grouping, Sort},
    ui,
};

#[test]
fn each_last_explicit_preference_survives_pane_switch_exit_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    let mut store = Store::load(path.clone()).unwrap();
    let mut app = App::new(root.path().to_path_buf());
    store.settings.apply(&mut app);
    app.dual = true;
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        store.settings.clone(),
    );
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    controller.dispatch(Action::Sort, &geometry);
    controller.dispatch(Action::Hidden, &geometry);
    controller.dispatch(Action::Group, &geometry);
    controller.dispatch(Action::FocusPane, &geometry);
    assert_eq!(controller.app.panes[1].sort.sort, Sort::Natural);
    assert!(!controller.app.panes[1].show_hidden);
    controller.dispatch(Action::Reverse, &geometry);
    controller.dispatch(Action::Ignore, &geometry);
    controller.dispatch(Action::ToggleSidebar, &geometry);
    controller.dispatch(Action::FocusPane, &geometry);
    assert!(!controller.app.panes[0].sort.reverse);
    controller.dispatch(Action::Quit, &geometry);
    assert!(controller.quit && controller.preferences_changed);
    store.settings = controller.preferences;
    store.save().unwrap();
    let restored = Store::load(path).unwrap();
    let mut restarted = App::new(root.path().to_path_buf());
    restored.settings.apply(&mut restarted);
    assert!(!restarted.dual);
    assert!(restarted.sidebar);
    for pane in &restarted.panes {
        assert_eq!(pane.sort.sort, Sort::Name);
        assert!(pane.show_hidden);
        assert!(pane.sort.reverse);
        assert_eq!(pane.sort.grouping, Grouping::Last);
        assert_eq!(pane.ignore, IgnoreMode::Hide);
    }
}

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
