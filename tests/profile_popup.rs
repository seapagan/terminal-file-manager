//! Profile content and mouse targets share the bounded popup viewport.
use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect, style::Modifier};
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

fn area_text(buffer: &Buffer, area: Rect) -> String {
    (area.y..area.bottom())
        .flat_map(|y| (area.x..area.right()).map(move |x| buffer[(x, y)].symbol()))
        .collect::<String>()
        .split_whitespace()
        .collect()
}

fn rendered_rows(buffer: &Buffer, area: Rect) -> Vec<String> {
    (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

fn assert_feedback(buffer: &Buffer, body: Rect, expected: &str, style: Modifier) {
    let rows = rendered_rows(buffer, body);
    let start = rows
        .iter()
        .position(|row| row.starts_with(expected.split_whitespace().next().unwrap()))
        .unwrap();
    assert_eq!(start, 2, "Name, one blank row, then feedback: {rows:?}");
    assert!(rows[1].trim().is_empty());
    assert!(rows.last().unwrap().trim().is_empty());
    let feedback = rows[start..]
        .iter()
        .map(|row| row.trim())
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(feedback, expected);
    assert!(
        buffer[(body.x, body.y + start as u16)]
            .modifier
            .intersection(Modifier::BOLD | Modifier::DIM)
            == style
    );
}

#[test]
fn save_profile_desktop_rows_group_the_field_feedback_and_separated_footer() {
    let mut c = editor();
    let (initial, geometry) = draw(&c, 120, 32);
    let shell = geometry.menu_layout.unwrap();
    assert_eq!(shell.outer, Rect::new(28, 10, 64, 10));
    assert_eq!(shell.body, Rect::new(30, 12, 60, 5));
    assert_eq!(shell.footer, Rect::new(30, 17, 60, 1));
    let area = Rect::new(shell.body.x, shell.body.y, shell.body.width, 6);
    assert_eq!(
        rendered_rows(&initial, area),
        [
            "Name: ▏",
            "",
            "Use 1–64 printable characters; no surrounding whitespace.",
            "",
            "",
            "Enter saves; Esc cancels",
        ]
    );
    key(&mut c, KeyCode::Enter, 120, 32);
    let (failed, geometry) = draw(&c, 120, 32);
    assert_eq!(geometry.menu_layout.unwrap().outer, shell.outer);
    assert_eq!(
        rendered_rows(&failed, area),
        [
            "Name: ▏",
            "",
            "profile name must be 1–64 printable characters without",
            "surrounding whitespace",
            "",
            "Enter saves; Esc cancels",
        ]
    );
    type_name(&mut c, "W");
    let (corrected, geometry) = draw(&c, 120, 32);
    assert_eq!(geometry.menu_layout.unwrap().outer, shell.outer);
    assert_eq!(rendered_rows(&corrected, area)[0], "Name: W▏");
    assert_eq!(
        &rendered_rows(&corrected, area)[1..],
        &rendered_rows(&initial, area)[1..]
    );
}

#[test]
fn save_profile_guidance_and_errors_wrap_at_words_with_spacing_and_semantic_styles() {
    for (width, height, body_height) in [(160, 48, 5), (120, 32, 5), (40, 24, 6), (32, 24, 7)] {
        let mut c = editor();
        let (buffer, geometry) = draw(&c, width, height);
        let shell = geometry.menu_layout.unwrap();
        assert_eq!(shell.outer.width, 64.min(width - 2));
        assert_eq!(shell.body.height, body_height);
        assert_eq!(shell.outer.height, body_height + 5);
        assert_eq!(shell.body.y, shell.outer.y + 2);
        assert_eq!(shell.footer.y, shell.body.bottom());
        assert!(area_text(&buffer, shell.footer).starts_with("Entersaves;Esccancels"));
        assert_feedback(
            &buffer,
            shell.body,
            "Use 1–64 printable characters; no surrounding whitespace.",
            Modifier::empty(),
        );
        key(&mut c, KeyCode::Enter, width, height);
        let (buffer, geometry) = draw(&c, width, height);
        let failed = geometry.menu_layout.unwrap();
        assert_eq!(
            (failed.outer, failed.body, failed.footer),
            (shell.outer, shell.body, shell.footer)
        );
        assert_feedback(
            &buffer,
            failed.body,
            c.app.profile_error.as_deref().unwrap(),
            Modifier::BOLD,
        );
        assert_eq!(c.app.profile_name.as_deref(), Some(""));
        assert!(c.app.notifications.history.is_empty());
    }
}

#[test]
fn save_profile_reserves_desktop_width_and_stable_feedback_geometry() {
    for (width, height) in [(160, 48), (120, 32), (80, 24), (40, 24), (32, 24)] {
        let mut c = editor();
        let (initial, geometry) = draw(&c, width, height);
        let shell = geometry.menu_layout.unwrap();
        assert_eq!(shell.outer.width, 64.min(width - 2));
        assert!(
            area_text(&initial, shell.body)
                .contains("Use1–64printablecharacters;nosurroundingwhitespace.")
        );
        for name in ["Work界", " padded ", &"界".repeat(64)] {
            c.app.profile_name = Some(name.into());
            c.app.profile_error = None;
            let (_, edited) = draw(&c, width, height);
            assert_eq!(edited.menu_layout.as_ref().unwrap().outer, shell.outer);
            if name.starts_with(' ') {
                key(&mut c, KeyCode::Enter, width, height);
                let (invalid, geometry) = draw(&c, width, height);
                let failed = geometry.menu_layout.unwrap();
                assert_eq!(
                    (failed.outer, failed.body, failed.footer),
                    (shell.outer, shell.body, shell.footer)
                );
                let text = area_text(&invalid, failed.body);
                assert!(text.contains("profilenamemustbe"));
                assert!(!text.contains("Use1–64"));
                key(&mut c, KeyCode::Backspace, width, height);
                let (correcting, geometry) = draw(&c, width, height);
                assert_eq!(geometry.menu_layout.unwrap().outer, shell.outer);
                assert!(area_text(&correcting, shell.body).contains("Use1–64"));
            }
        }
        assert!(c.app.notifications.history.is_empty());
        assert!(!c.preferences_changed);
    }
}

#[test]
fn profile_validation_is_rendered_inside_the_editor_under_status_pressure() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("marked"), "fixture").unwrap();
    let mut c = editor();
    c.app.panes[0].path = root.path().into();
    c.app.complete_listing(
        0,
        1,
        Ok(terminal_file_manager::fs::list(
            root.path(),
            &terminal_file_manager::fs::IgnoreContext::isolated(),
            &false.into(),
        )
        .unwrap()),
    );
    c.app.panes[0].select_all();
    assert_eq!(c.app.panes[0].marks.len(), 1);
    c.app.notify(
        terminal_file_manager::notifications::Severity::Error,
        "Retained failure",
    );
    c.app.notifications.dismiss_toasts();
    let status = c.app.status.clone();
    for name in ["", " padded "] {
        c.app.profile_name = Some(name.into());
        key(&mut c, KeyCode::Enter, 120, 24);
        let expected = terminal_file_manager::settings::validate_profile_name(name).unwrap_err();
        for width in [120, 40, 32, 24, 120] {
            let (buffer, geometry) = draw(&c, width, 24);
            let body = geometry.menu_layout.unwrap().body;
            let text = area_text(&buffer, body);
            assert!(
                text.contains(&expected.split_whitespace().collect::<String>()),
                "width {width}: {text}"
            );
            assert!(text.contains("Name:"));
            assert_eq!(c.app.profile_name.as_deref(), Some(name));
        }
        assert_eq!(c.app.status, status);
        assert_eq!(c.app.notifications.history.len(), 1);
        assert!(c.app.notifications.visible().is_empty());
        assert!(!c.preferences_changed);
        assert!(c.app.operations.outbox.is_empty());
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
        assert!(
            c.app
                .profile_error
                .as_ref()
                .unwrap()
                .contains("profile name must be")
        );
        assert!(c.app.profile_name.is_some() && c.app.profile_menu.is_some());
        assert_eq!(c.preferences.profiles, original);
        assert!(!c.preferences_changed);
    }
    key(&mut c, KeyCode::Backspace, 120, 24);
    assert!(c.app.profile_error.is_none());
    let (buffer, geometry) = draw(&c, 32, 24);
    assert!(!area_text(&buffer, geometry.menu_layout.unwrap().body).contains("profilename"));
    type_name(&mut c, "Wé");
    key(&mut c, KeyCode::Backspace, 120, 24);
    type_name(&mut c, "界");
    assert_eq!(c.app.profile_name.as_deref(), Some("W界"));
    key(&mut c, KeyCode::Enter, 120, 24);
    assert!(c.app.profile_name.is_none() && c.app.profile_menu.is_none());
    assert!(c.app.profile_error.is_none());
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
    type_name(&mut c, "Draft界 ");
    key(&mut c, KeyCode::Enter, 32, 24);
    assert!(c.app.profile_error.is_some());
    key(&mut c, KeyCode::Esc, 120, 24);
    assert!(c.app.profile_name.is_none() && c.app.profile_menu.is_some());
    assert!(c.app.profile_error.is_none());
    key(&mut c, KeyCode::Enter, 32, 24);
    assert_eq!(c.app.profile_name.as_deref(), Some(""));
    let (buffer, geometry) = draw(&c, 32, 24);
    assert!(!area_text(&buffer, geometry.menu_layout.unwrap().body).contains("profilename"));
    key(&mut c, KeyCode::Esc, 32, 24);
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
fn unicode_name_editor_keeps_the_editing_end_on_one_row_after_resize() {
    let mut c = editor();
    let name = format!("{}尾", "界e\u{301}".repeat(20));
    type_name(&mut c, &name);
    for width in [120, 40, 32, 24, 120] {
        let (buffer, geometry) = draw(&c, width, 24);
        let body = geometry.menu_layout.as_ref().unwrap().body;
        let area = geometry.menu.unwrap();
        assert!(area.x > 0 && area.right() < width && area.bottom() < 24);
        let field = area_text(&buffer, Rect::new(body.x, body.y, body.width, 1));
        assert!(field.starts_with("Name:") && field.ends_with("尾▏"));
        assert_eq!(c.app.profile_name.as_deref(), Some(name.as_str()));
        assert!(area_text(&buffer, Rect::new(body.x, body.y + 1, body.width, 1)).is_empty());
        for y in area.y + 1..area.bottom() - 1 {
            assert_eq!(buffer[(area.x, y)].symbol(), "│");
            assert_eq!(buffer[(area.right() - 1, y)].symbol(), "│");
        }
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
            let body = g.menu_layout.as_ref().unwrap().body;
            click(&mut c, &g, body.x, body.y + 1);
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
    let body = g.menu_layout.as_ref().unwrap().body;
    let row: String = (body.x..body.right())
        .map(|x| buffer[(x, body.y)].symbol())
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
        let body = g.menu_layout.as_ref().unwrap().body;
        let row: String = (body.x..body.right())
            .map(|x| buffer[(x, body.y + 1)].symbol())
            .collect();
        // TestBackend retains padding cells after wide graphemes.
        let expected = expected.replace('界', "界 ");
        assert_eq!(row.trim_end(), expected);
        click(&mut c, &g, area.right() - 2, area.y + 2);
        assert_eq!(c.app.profile_menu, Some(1));
    }
}
