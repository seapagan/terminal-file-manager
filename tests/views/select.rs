use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Modifier};
use terminal_file_manager::{color::Theme, input::Controller, ui};

pub fn option(c: &mut Controller, label: &str, bounds: Rect) -> Rect {
    let g = ui::geometry(&c.app, bounds);
    while c.app.view_menu.is_some_and(|selected| selected > 0) {
        c.handle(
            Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            &g,
            Default::default(),
        );
    }
    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    loop {
        let mut g = ui::geometry(&c.app, bounds);
        terminal
            .draw(|frame| {
                g = ui::render(
                    frame,
                    &c.app,
                    &Theme::new(colored_text::ColorLevel::NoColor),
                );
            })
            .unwrap();
        let body = g.menu_layout.as_ref().unwrap().body;
        let buffer = terminal.backend().buffer();
        for y in body.y..body.bottom() {
            let row: String = (body.x..body.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            if row.trim_start().starts_with(label)
                && buffer[(body.x, y)].modifier.contains(Modifier::REVERSED)
            {
                return Rect::new(body.x, y, body.width, 1);
            }
        }
        let before = c.app.view_menu;
        c.handle(
            Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            &g,
            Default::default(),
        );
        assert_ne!(c.app.view_menu, before, "View option not found: {label}");
    }
}
