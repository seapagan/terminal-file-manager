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
fn explicit_detail_columns_survive_exit_from_the_other_pane_and_restart() {
    use terminal_file_manager::view::Columns;
    for (action, flags) in [
        (Action::ColumnModified, [false, true, true]),
        (Action::ColumnAttributes, [true, false, true]),
        (Action::ColumnOwner, [true, true, false]),
    ] {
        let expected = Columns {
            modified: flags[0],
            attributes: flags[1],
            owner: flags[2],
            ..Columns::default()
        };
        let root = tempfile::tempdir().unwrap();
        let mut store = Store::load(root.path().join("config.toml")).unwrap();
        let mut app = App::new(root.path().into());
        store.settings.apply(&mut app);
        app.dual = true;
        let mut c = Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            store.settings.clone(),
        );
        let geometry = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 160, 24));
        c.dispatch(action, &geometry);
        assert_eq!(c.app.panes[0].columns, expected);
        assert_eq!(c.app.panes[1].columns, Columns::default());
        assert_eq!(c.preferences.columns, expected);
        assert!(c.preferences_changed);
        c.dispatch(Action::FocusPane, &geometry);
        assert_eq!(c.app.active, 1);
        c.dispatch(Action::Quit, &geometry);
        assert!(c.quit);
        store.settings = c.preferences;
        store.save().unwrap();
        let restored = Store::load(store.path).unwrap();
        let mut restarted = App::new(root.path().into());
        restored.settings.apply(&mut restarted);
        assert!(restarted.panes.iter().all(|pane| pane.columns == expected));
        assert_eq!(restored.settings.profiles, Settings::default().profiles);
    }
}

#[test]
fn clean_settings_and_every_builtin_profile_start_without_reverse() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    let store = Store::load(path.clone()).unwrap();
    assert!(!path.exists());
    assert!(!store.settings.reverse);
    assert!(!terminal_file_manager::listing::SortPolicy::default().reverse);
    assert!(!terminal_file_manager::view::ViewSettings::default().reverse);
    let mut app = App::new(root.path().into());
    store.settings.apply(&mut app);
    assert!(app.panes.iter().all(|pane| !pane.sort.reverse));
    for profile in store.settings.profiles.values() {
        app.panes[0].sort.reverse = true;
        profile.apply(&mut app.panes[0]);
        assert!(!app.panes[0].sort.reverse);
    }
}

#[test]
fn saved_reverse_true_and_explicit_false_survive_restart_and_focus_changes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    let mut store = Store::load(path.clone()).unwrap();
    store.settings.reverse = true;
    store.save().unwrap();
    store = Store::load(path.clone()).unwrap();
    let mut app = App::new(root.path().into());
    store.settings.apply(&mut app);
    assert!(app.panes.iter().all(|pane| pane.sort.reverse));
    app.dual = true;
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        store.settings,
    );
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    controller.dispatch(Action::Reverse, &geometry);
    controller.dispatch(Action::FocusPane, &geometry);
    assert!(controller.app.panes[1].sort.reverse);
    assert!(!controller.preferences.reverse);
    store.settings = controller.preferences;
    store.save().unwrap();
    let restored = Store::load(path).unwrap();
    restored.settings.apply(&mut controller.app);
    assert!(controller.app.panes.iter().all(|pane| !pane.sort.reverse));
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
fn invalid_utf8_config_fails_without_replacing_bytes_with_defaults() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    let bytes = b"mouse = false\n\xff\xfe";
    fs::write(&path, bytes).unwrap();
    let error = Store::load(path.clone())
        .err()
        .expect("invalid UTF-8 must fail");
    assert!(error.contains(&path.display().to_string()));
    assert!(error.to_ascii_lowercase().contains("utf-8"));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn linked_config_loads_but_saving_refuses_to_replace_the_link_or_target() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    fs::write(&target, "mouse = false\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, root.path().join("config.toml")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&target, root.path().join("config.toml")).unwrap();
    let path = root.path().join("config.toml");
    let store = Store::load(path.clone()).unwrap();
    assert!(!store.settings.mouse);
    assert!(store.save().unwrap_err().contains("not a link"));
    assert!(fs::symlink_metadata(path).unwrap().is_symlink());
    assert_eq!(fs::read_to_string(target).unwrap(), "mouse = false\n");
}

fn config_link(target: &std::path::Path, link: &std::path::Path, directory: bool) {
    #[cfg(unix)]
    {
        let _ = directory;
        std::os::unix::fs::symlink(target, link).unwrap();
    }
    #[cfg(windows)]
    if directory {
        std::os::windows::fs::symlink_dir(target, link).unwrap();
    } else {
        std::os::windows::fs::symlink_file(target, link).unwrap();
    }
}

#[test]
fn dangling_and_nonregular_config_links_fail_without_defaults_or_writes() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    let link = root.path().join("config.toml");
    config_link(&target, &link, false);
    assert!(Store::load(link.clone()).is_err());
    let store = Store {
        path: link.clone(),
        settings: Settings::default(),
    };
    assert!(store.save().is_err());
    assert!(!target.exists());
    fs::remove_file(&link).unwrap();
    fs::create_dir(&target).unwrap();
    config_link(&target, &link, true);
    assert!(Store::load(link.clone()).is_err());
    assert!(store.save().is_err());
    assert!(fs::symlink_metadata(link).unwrap().is_symlink());
    assert!(fs::read_dir(target).unwrap().next().is_none());
}

#[test]
fn regular_and_linked_config_reads_are_bounded_and_strict() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    let link = root.path().join("config.toml");
    config_link(&target, &link, false);
    let mut text = "mouse = false\n#".to_owned();
    text.push_str(&" ".repeat(1_048_576 - text.len()));
    fs::write(&target, &text).unwrap();
    assert!(!Store::load(target.clone()).unwrap().settings.mouse);
    text.push(' ');
    fs::write(&target, &text).unwrap();
    for path in [&target, &link] {
        let error = Store::load(path.clone()).err().expect("oversize must fail");
        assert!(error.contains("config exceeds 1 MiB"));
    }
    text.pop();
    fs::write(&target, &text).unwrap();
    assert!(!Store::load(link.clone()).unwrap().settings.mouse);
    for bytes in [b"unknown = true".as_slice(), b"mouse = false\n\xff"] {
        fs::write(&target, bytes).unwrap();
        assert!(Store::load(link.clone()).is_err());
        assert_eq!(fs::read(&target).unwrap(), bytes);
    }
}
