//! View settings and menu behaviour remain independent of navigation state.
use crossterm::event::{Event, KeyEvent};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    input::{Bindings, Controller, chord},
    settings::Settings,
    ui,
};

fn key(controller: &mut Controller, text: &str) {
    let (code, modifiers) = chord(text).unwrap();
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    controller.handle(
        Event::Key(KeyEvent::new(code, modifiers)),
        &geometry,
        Duration::ZERO,
    );
}

#[test]
fn view_menu_changes_active_formatting_and_preserves_last_explicit_defaults() {
    let mut controller = Controller::new(
        App::new(".".into()),
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    key(&mut controller, "V");
    key(&mut controller, "Space");
    assert!(!controller.app.panes[0].human_sizes);
    assert!(controller.app.panes[1].human_sizes);
    assert!(!controller.preferences.human_sizes);
    key(&mut controller, "Down");
    key(&mut controller, "Enter");
    assert!(controller.app.panes[0].fuzzy_timestamps);
    key(&mut controller, "Esc");
    key(&mut controller, "h");
    assert!(controller.app.panes[0].show_hidden);
    assert!(controller.preferences.hidden);
    assert!(controller.app.view_menu.is_none());
}

#[test]
fn profiles_apply_only_view_settings_and_edits_do_not_mutate_saved_profiles() {
    let root = tempfile::tempdir().unwrap();
    let settings = Settings::default();
    let saved = settings.profiles["Detailed"].clone();
    let mut controller = Controller::new(
        App::new(root.path().into()),
        Bindings::new(&Default::default()).unwrap(),
        settings,
    );
    key(&mut controller, "Alt+v");
    key(&mut controller, "Down");
    key(&mut controller, "Enter");
    assert_eq!(controller.app.panes[0].profile.as_deref(), Some("Detailed"));
    assert!(controller.app.panes[0].fuzzy_timestamps);
    assert!(!controller.app.panes[1].fuzzy_timestamps);
    assert_eq!(controller.app.panes[0].path, root.path());
    key(&mut controller, "h");
    assert!(controller.app.panes[0].profile_modified);
    assert_eq!(controller.preferences.profiles["Detailed"], saved);
    assert_eq!(
        controller.app.panes[0].profile_label(),
        "Modified: Detailed"
    );
}

#[test]
fn profiles_save_explicitly_round_trip_and_apply_to_both_without_locations() {
    let root = tempfile::tempdir().unwrap();
    let opposite = root.path().join("other");
    std::fs::create_dir(&opposite).unwrap();
    let mut controller = Controller::new(
        App::new(root.path().into()),
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    controller.app.panes[1].path = opposite.clone();
    key(&mut controller, "h");
    key(&mut controller, "Alt+v");
    key(&mut controller, "End");
    key(&mut controller, "Enter");
    for letter in ["W", "o", "r", "k"] {
        key(&mut controller, letter);
    }
    key(&mut controller, "Enter");
    assert!(controller.preferences.profiles["Work"].hidden);
    let store = terminal_file_manager::settings::Store {
        path: root.path().join("config.toml"),
        settings: controller.preferences.clone(),
    };
    store.save().unwrap();
    assert_eq!(
        terminal_file_manager::settings::Store::load(store.path)
            .unwrap()
            .settings
            .profiles["Work"],
        controller.preferences.profiles["Work"]
    );
    key(&mut controller, "Alt+v");
    key(&mut controller, "Down");
    key(&mut controller, "Space");
    key(&mut controller, "Enter");
    assert_eq!(controller.app.panes[0].path, root.path());
    assert_eq!(controller.app.panes[1].path, opposite);
    for pane in &controller.app.panes {
        assert_eq!(pane.profile.as_deref(), Some("Detailed"));
        assert!(pane.fuzzy_timestamps);
        assert!(!pane.profile_modified);
    }
}

#[test]
fn profiles_reject_navigation_fields_and_invalid_names() {
    for text in [
        "[profiles.Work]\npath='/tmp'",
        "[profiles.Work]\ncursor=4",
        "[profiles.' ']\nhuman_sizes=false",
    ] {
        assert!(Settings::parse(text).is_err());
    }
    let settings =
        Settings::parse("[profiles.Work]\nhuman_sizes=false\n[profiles.Work.columns]\nowner=false")
            .unwrap();
    assert!(!settings.profiles["Work"].human_sizes);
    assert!(settings.profiles.contains_key("Compact"));
    assert!(settings.profiles.contains_key("Detailed"));
    assert!(settings.profiles.contains_key("Minimal"));
}

#[test]
fn view_menu_mouse_uses_drawn_geometry_and_column_settings_round_trip() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let mut controller = Controller::new(
        App::new(".".into()),
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    key(&mut controller, "V");
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    let area = geometry.menu.unwrap();
    controller.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x + 2,
            row: area.y + 9,
            modifiers: KeyModifiers::NONE,
        }),
        &geometry,
        Duration::ZERO,
    );
    assert!(!controller.app.panes[0].columns.size);
    assert!(!controller.preferences.columns.size);
    let text = toml::to_string(&controller.preferences).unwrap();
    assert!(!Settings::parse(&text).unwrap().columns.size);
    assert!(Settings::parse("sidebar = true").unwrap().columns.size);
}

#[test]
fn column_headers_and_cells_hide_together_without_losing_name_or_type() {
    use colored_text::ColorLevel;
    use ratatui::{Terminal, backend::TestBackend};
    use std::sync::atomic::AtomicBool;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("name"), "one").unwrap();
    let mut app = App::new(root.path().into());
    app.complete_listing(
        0,
        1,
        Ok(terminal_file_manager::fs::list(
            root.path(),
            &terminal_file_manager::fs::IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    let settings =
        Settings::parse("[columns]\nsize=false\nmodified=false\nattributes=false\nowner=false")
            .unwrap();
    settings.apply(&mut app);
    for width in [7, 25, 80, 160] {
        let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
        terminal
            .draw(|frame| {
                ui::render(
                    frame,
                    &app,
                    &terminal_file_manager::color::Theme::new(ColorLevel::NoColor),
                );
            })
            .unwrap();
        let rows = terminal
            .backend()
            .buffer()
            .content()
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>();
        assert!(!rows[1].contains("Size"));
        assert!(!rows[1].contains("Modified"));
        if width >= 25 {
            assert!(rows[2].contains("name"));
        }
    }
}

#[test]
fn overwriting_a_profile_reconciles_other_badges_without_changing_their_view() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = Controller::new(
        App::new(root.path().into()),
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    for input in ["Alt+v", "Down", "Space", "Enter"] {
        key(&mut controller, input);
    }
    let other = terminal_file_manager::view::ViewSettings::capture(&controller.app.panes[1]);
    key(&mut controller, "h");
    for input in ["Alt+v", "End", "Enter", "Enter"] {
        key(&mut controller, input);
    }
    assert_eq!(controller.app.panes[0].profile_label(), "Detailed");
    assert_eq!(
        controller.app.panes[1].profile_label(),
        "Modified: Detailed"
    );
    assert_eq!(
        terminal_file_manager::view::ViewSettings::capture(&controller.app.panes[1]),
        other
    );
    assert_eq!(controller.app.panes[1].path, root.path());
    key(&mut controller, "h");
    for input in ["Alt+v", "End", "Enter", "Enter"] {
        key(&mut controller, input);
    }
    assert_eq!(controller.app.panes[1].profile_label(), "Detailed");
    assert!(!controller.app.panes[1].profile_modified);
}
