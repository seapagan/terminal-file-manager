mod decisions;
mod presentation;
mod versions;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Bindings, Controller},
    settings::Settings,
    ui,
};

fn controller(text: &str, running: &str) -> Controller {
    let settings = Settings::parse(text).unwrap();
    let mut c = Controller::new(
        App::new(".".into()),
        Bindings::new(&settings.bindings).unwrap(),
        settings,
    );
    c.check_compound_extensions(running).unwrap();
    c
}

fn event(c: &mut Controller, event: Event, width: u16, height: u16) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, width, height));
    assert!(c.handle(event, &g, std::time::Duration::ZERO).is_empty());
}

fn key(c: &mut Controller, code: KeyCode) {
    event(
        c,
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        120,
        40,
    );
}

fn draw(c: &Controller, width: u16, height: u16) -> (String, ui::Geometry) {
    let (t, geometry) = canvas(c, width, height, colored_text::ColorLevel::NoColor);
    let buffer = t.backend().buffer();
    let text = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (text, geometry)
}

fn canvas(
    c: &Controller,
    width: u16,
    height: u16,
    level: colored_text::ColorLevel,
) -> (Terminal<TestBackend>, ui::Geometry) {
    let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut geometry = None;
    t.draw(|frame| geometry = Some(ui::render(frame, &c.app, &Theme::new(level))))
        .unwrap();
    (t, geometry.unwrap())
}
