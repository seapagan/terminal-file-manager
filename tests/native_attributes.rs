//! Native attribute strings remain visible without a Unix formatting control.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs,
    input::{Action, Bindings, Controller},
    permissions::PermissionDisplay,
    ui,
};

fn position(buffer: &Buffer, text: &str) -> (u16, u16) {
    for y in 0..buffer.area.height {
        let row: String = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect();
        if let Some(x) = row.find(text) {
            return (row[..x].chars().count() as u16, y);
        }
    }
    panic!("missing attribute text: {text}");
}

#[test]
fn native_attributes_and_wrapped_continuations_are_read_only_in_both_views() {
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(root.path().into());
    let mut entry = fs::inspect(root.path());
    entry.mode = None;
    entry.attributes = "readonly, hidden".into();
    app.details[0].entry = Some(entry);
    app.sidebar = true;
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    for (floating, width) in [(false, 120), (true, 120), (true, 24)] {
        c.app.popup = floating;
        let bounds = Rect::new(0, 0, width, 80);
        let mut terminal = Terminal::new(TestBackend::new(width, 80)).unwrap();
        terminal
            .draw(|f| {
                ui::render(f, &c.app, &Theme::new(ColorLevel::NoColor));
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let g = ui::geometry(&c.app, bounds);
        assert!(
            g.metadata_controls
                .iter()
                .all(|control| control.action != Action::Permissions)
        );
        for text in ["readonly,", "hidden"] {
            let (x, y) = position(buffer, text);
            c.handle(
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: x,
                    row: y,
                    modifiers: KeyModifiers::NONE,
                }),
                &g,
                Duration::ZERO,
            );
            assert_eq!(
                c.app.metadata_format(floating).permissions,
                PermissionDisplay::Symbolic
            );
            assert!(!c.preferences_changed);
        }
    }
}
