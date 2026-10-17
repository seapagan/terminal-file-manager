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
    input::{Bindings, Controller},
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
