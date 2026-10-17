//! Profiles define sidebar preference independently of temporary layout geometry.
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    input::{Action, Bindings, Controller},
    settings::Settings,
    ui,
    view::ViewSettings,
};

fn controller(sidebar: bool) -> Controller {
    let settings = Settings {
        sidebar,
        ..Default::default()
    };
    let mut app = App::new(".".into());
    settings.apply(&mut app);
    Controller::new(app, Bindings::new(&Default::default()).unwrap(), settings)
}

fn action(c: &mut Controller, action: Action) {
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    c.dispatch(action, &geometry);
}

fn profile(c: &mut Controller, index: usize) {
    action(c, Action::ViewProfiles);
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    for code in std::iter::repeat_n(KeyCode::Down, index).chain([KeyCode::Enter]) {
        c.handle(
            Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            &geometry,
            Duration::ZERO,
        );
    }
}

#[test]
fn builtin_profiles_have_repeatable_sidebar_results_from_both_initial_states() {
    for initial in [false, true] {
        let mut c = controller(initial);
        c.app.panes[0].show_hidden = initial;
        for (index, expected) in [(1, true), (0, false), (1, true), (2, false), (0, false)] {
            profile(&mut c, index);
            assert_eq!(c.app.sidebar, expected);
            assert_eq!(c.app.panes[0].sidebar, expected);
            assert_eq!(c.preferences.sidebar, expected);
            assert_eq!(c.app.panes[0].show_hidden, expected);
            assert_eq!(c.preferences.hidden, expected);
            assert!(!c.app.panes[1].show_hidden);
            assert_eq!(c.app.panes[1].sidebar, initial);
            assert_eq!(
                ui::geometry(&c.app, Rect::new(0, 0, 120, 24))
                    .sidebar
                    .is_some(),
                expected
            );
        }
    }
}

#[test]
fn focus_maximise_resize_and_popup_preserve_each_panes_sidebar_preference() {
    let mut c = controller(false);
    profile(&mut c, 1);
    assert!(c.app.panes[0].show_hidden);
    action(&mut c, Action::TogglePanes);
    action(&mut c, Action::Maximise);
    assert!(c.app.sidebar);
    assert!(
        ui::geometry(&c.app, Rect::new(0, 0, 120, 24))
            .sidebar
            .is_none()
    );
    action(&mut c, Action::Maximise);
    action(&mut c, Action::FocusPane);
    assert!(!c.app.sidebar);
    assert!(!c.app.panes[1].show_hidden);
    profile(&mut c, 1);
    assert!(c.app.panes[1].show_hidden);
    action(&mut c, Action::TogglePanes);
    action(&mut c, Action::Metadata);
    assert!(c.app.popup && c.app.sidebar);
    for (width, shown) in [(30, false), (120, true)] {
        assert_eq!(
            ui::geometry(&c.app, Rect::new(0, 0, width, 24))
                .sidebar
                .is_some(),
            shown
        );
        assert!(c.app.panes[1].sidebar);
    }
    let restored = Settings::parse(&toml::to_string(&c.preferences).unwrap()).unwrap();
    let mut restart = App::new(".".into());
    restored.apply(&mut restart);
    assert!(restart.sidebar && restart.panes.iter().all(|p| p.sidebar));
    assert!(restart.panes.iter().all(|p| p.show_hidden));
}

#[test]
fn user_profile_omission_inherits_but_explicit_save_captures_sidebar() {
    for initial in [false, true] {
        let mut c = controller(initial);
        ViewSettings::default().apply(&mut c.app.panes[0]);
        assert_eq!(c.app.panes[0].sidebar, initial);
        let saved = ViewSettings::capture(&c.app.panes[0]);
        assert_eq!(saved.sidebar, Some(initial));
        assert_eq!(c.app.panes[0].profile_label(), "Custom");
    }
}
