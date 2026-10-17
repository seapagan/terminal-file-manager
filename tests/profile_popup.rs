//! Profile content and mouse targets share the bounded popup viewport.
use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Action, Bindings, Controller},
    settings::Settings,
    ui,
};

fn controller() -> Controller {
    let mut c = Controller::new(
        App::new(".".into()),
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    c.app.profile_menu = Some(0);
    c
}
fn draw(c: &Controller, width: u16, height: u16) -> (Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut geometry = None;
    terminal
        .draw(|f| geometry = Some(ui::render(f, &c.app, &Theme::new(ColorLevel::NoColor))))
        .unwrap();
    (terminal.backend().buffer().clone(), geometry.unwrap())
}
fn key(c: &mut Controller, code: KeyCode, width: u16, height: u16) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, width, height));
    c.handle(
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        &g,
        Duration::ZERO,
    );
}
fn click(c: &mut Controller, g: &ui::Geometry, x: u16, y: u16) {
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }),
        g,
        Duration::ZERO,
    );
}

fn editor() -> Controller {
    let mut c = controller();
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    c.dispatch(Action::ViewProfiles, &g);
    key(&mut c, KeyCode::End, 120, 24);
    key(&mut c, KeyCode::Enter, 120, 24);
    assert_eq!(c.app.profile_name.as_deref(), Some(""));
    c
}

fn type_name(c: &mut Controller, name: &str) {
    for character in name.chars() {
        key(c, KeyCode::Char(character), 120, 24);
    }
}

#[test]
fn invalid_profile_submission_can_be_corrected_with_unicode_and_saved_explicitly() {
    let root = tempfile::tempdir().unwrap();
    let mut c = editor();
    let original = c.preferences.profiles.clone();
    let view = terminal_file_manager::view::ViewSettings::capture(&c.app.panes[0]);
    for invalid in [None, Some(' ')] {
        if let Some(character) = invalid {
            key(&mut c, KeyCode::Char(character), 120, 24);
        }
        key(&mut c, KeyCode::Enter, 120, 24);
        assert!(c.app.status.contains("profile name must be"));
        assert!(c.app.profile_name.is_some() && c.app.profile_menu.is_some());
        assert_eq!(c.preferences.profiles, original);
        assert!(!c.preferences_changed);
    }
    key(&mut c, KeyCode::Backspace, 120, 24);
    type_name(&mut c, "Wé");
    key(&mut c, KeyCode::Backspace, 120, 24);
    type_name(&mut c, "界");
    assert_eq!(c.app.profile_name.as_deref(), Some("W界"));
    key(&mut c, KeyCode::Enter, 120, 24);
    assert!(c.app.profile_name.is_none() && c.app.profile_menu.is_none());
    assert_eq!(c.preferences.profiles["W界"], view);
    assert_eq!(c.app.panes[0].profile.as_deref(), Some("W界"));
    assert!(c.app.panes[1].profile.is_none());
    assert!(c.preferences_changed);
    let store = terminal_file_manager::settings::Store {
        path: root.path().join("config.toml"),
        settings: c.preferences,
    };
    store.save().unwrap();
    let restored = terminal_file_manager::settings::Store::load(store.path).unwrap();
    assert_eq!(restored.settings.profiles["W界"], view);
    for (name, profile) in original {
        assert_eq!(restored.settings.profiles[&name], profile);
    }
}

#[test]
fn cancelling_profile_name_editing_preserves_views_profiles_and_persistence_state() {
    let mut c = editor();
    let original = toml::to_string(&c.preferences).unwrap();
    let views = c
        .app
        .panes
        .each_ref()
        .map(terminal_file_manager::view::ViewSettings::capture);
    type_name(&mut c, "Draft界");
    key(&mut c, KeyCode::Esc, 120, 24);
    assert!(c.app.profile_name.is_none() && c.app.profile_menu.is_some());
    key(&mut c, KeyCode::Esc, 120, 24);
    assert!(c.app.profile_menu.is_none());
    assert_eq!(toml::to_string(&c.preferences).unwrap(), original);
    assert!(!c.preferences_changed);
    for (pane, view) in c.app.panes.iter().zip(views) {
        assert_eq!(
            terminal_file_manager::view::ViewSettings::capture(pane),
            view
        );
        assert!(pane.profile.is_none() && !pane.profile_modified);
    }
}

#[test]
fn unicode_name_editor_wraps_within_its_popup_after_resize() {
    let mut c = editor();
    type_name(&mut c, "界é界é界é");
    for width in [120, 40, 24, 120] {
        let (buffer, geometry) = draw(&c, width, 24);
        let area = geometry.menu.unwrap();
        assert!(area.x > 0 && area.right() < width && area.bottom() < 24);
        let mut text = String::new();
        for y in area.y + 1..area.bottom() - 1 {
            assert_eq!(buffer[(area.x, y)].symbol(), "│");
            assert_eq!(buffer[(area.right() - 1, y)].symbol(), "│");
            text.extend((area.x + 2..area.right() - 2).map(|x| buffer[(x, y)].symbol().trim()));
        }
        assert!(text.contains("Name:界é界é界é"));
        c.app.profile_menu = None;
        let (background, _) = draw(&c, width, 24);
        c.app.profile_menu = Some(c.app.profile_names.len() + 1);
        for y in 0..24 {
            for x in 0..width {
                if !area.contains((x, y).into()) {
                    assert_eq!(buffer[(x, y)], background[(x, y)]);
                }
            }
        }
    }
}
#[test]
fn profile_popup_has_one_instruction_and_intact_padding_and_borders() {
    let c = controller();
    let (buffer, geometry) = draw(&c, 120, 24);
    let area = geometry.menu.unwrap();
    let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert_eq!(text.matches("Enter: active pane").count(), 1);
    assert_eq!(text.matches("Space: both").count(), 1);
    for y in area.y + 1..area.bottom() - 1 {
        assert_eq!(buffer[(area.x, y)].symbol(), "│");
        assert_eq!(buffer[(area.right() - 1, y)].symbol(), "│");
        assert_eq!(buffer[(area.x + 1, y)].symbol(), " ");
        assert_eq!(buffer[(area.right() - 2, y)].symbol(), " ");
    }
}
#[test]
fn profiles_resize_scroll_unicode_and_clicks_stay_in_the_rendered_viewport() {
    let mut c = controller();
    for i in 0..30 {
        c.preferences.profiles.insert(
            format!("界e\u{301}{i:02}{}", "長".repeat(40)),
            Default::default(),
        );
    }
    c.app.profile_names = c.preferences.profiles.keys().cloned().collect();
    for (w, h) in [(160, 30), (45, 14), (12, 8), (3, 3), (1, 1), (80, 20)] {
        key(&mut c, KeyCode::End, w, h);
        let (buffer, g) = draw(&c, w, h);
        let area = g.menu.unwrap();
        assert_eq!(area.intersection(Rect::new(0, 0, w, h)), area);
        if w >= 45 && h >= 14 {
            let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
            assert!(text.contains("Save active as…"));
            click(&mut c, &g, area.x, area.y + 1);
            assert_eq!(c.app.profile_menu, Some(c.app.profile_names.len() + 1));
            click(&mut c, &g, area.x + 1, area.y + 1);
            assert_eq!(c.app.profile_menu, Some(c.app.profile_names.len() + 1));
            key(&mut c, KeyCode::Home, w, h);
            let (_, g) = draw(&c, w, h);
            click(&mut c, &g, area.x + 2, area.y + 2);
            assert_eq!(c.app.profile_menu, Some(1));
            key(&mut c, KeyCode::Enter, w, h);
            assert_eq!(c.app.panes[0].profile.as_deref(), Some("Detailed"));
            c.app.profile_menu = Some(0);
        }
    }
}

#[test]
fn long_profile_names_truncate_without_drawing_or_clicking_outside_content() {
    let mut c = controller();
    c.app.profile_names = vec![format!("界e\u{301}{}", "長".repeat(60))];
    let (buffer, g) = draw(&c, 40, 16);
    let area = g.menu.unwrap();
    let row: String = (area.x + 2..area.right() - 2)
        .map(|x| buffer[(x, area.y + 1)].symbol())
        .collect();
    assert!(row.starts_with("界 e\u{301}"));
    assert!(row.trim_end().ends_with('…'));
    click(&mut c, &g, area.x + 2, area.bottom() - 2);
    assert_eq!(c.app.profile_menu, Some(0));
    c.app.profile_menu = None;
    let (background, _) = draw(&c, 40, 16);
    for y in 0..16 {
        for x in 0..40 {
            if !area.contains((x, y).into()) {
                assert_eq!(buffer[(x, y)], background[(x, y)]);
            }
        }
    }
}

#[test]
fn confirmation_row_fits_long_names_when_terminal_width_allows() {
    for name in ["x".repeat(60), "界e\u{301}".repeat(20)] {
        let mut c = controller();
        c.app.profile_names = vec![name.clone()];
        c.app.profile_menu = Some(1);
        let expected = format!("Apply selected to both: {name}");
        let (buffer, g) = draw(&c, 120, 24);
        let area = g.menu.unwrap();
        let row: String = (area.x + 2..area.right() - 2)
            .map(|x| buffer[(x, area.y + 2)].symbol())
            .collect();
        // TestBackend retains padding cells after wide graphemes.
        let expected = expected.replace('界', "界 ");
        assert_eq!(row.trim_end(), expected);
        click(&mut c, &g, area.right() - 2, area.y + 2);
        assert_eq!(c.app.profile_menu, Some(1));
    }
}
