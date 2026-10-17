//! Overlay input remains responsive at content bounds and after resize.
use crossterm::event::{Event, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    input::{Action, Bindings, Controller},
    ui,
};

fn controller(metadata: bool) -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(root.path().to_owned());
    if metadata {
        app.popup = true;
        app.details[0].totals = Some(Err("many wrapped metadata details ".repeat(100)));
    } else {
        app.help = true;
    }
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        ),
    )
}

fn draw(controller: &Controller, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(
                frame,
                &controller.app,
                &terminal_file_manager::color::Theme::new(colored_text::ColorLevel::NoColor),
            );
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn end_then_up_moves_immediately_in_help_and_metadata_after_excess_scroll() {
    for metadata in [false, true] {
        let (_root, mut controller) = controller(metadata);
        let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 80, 10));
        controller.dispatch(Action::End, &geometry);
        let before = draw(&controller, 80, 10);
        controller.dispatch(Action::Up, &geometry);
        assert_ne!(before, draw(&controller, 80, 10));
        let event = |kind| {
            Event::Mouse(MouseEvent {
                kind,
                column: 40,
                row: 5,
                modifiers: KeyModifiers::NONE,
            })
        };
        for _ in 0..1000 {
            controller.handle(event(MouseEventKind::ScrollDown), &geometry, Duration::ZERO);
        }
        let before = draw(&controller, 80, 10);
        controller.handle(event(MouseEventKind::ScrollUp), &geometry, Duration::ZERO);
        assert_ne!(before, draw(&controller, 80, 10));
        controller.handle(Event::Resize(120, 30), &geometry, Duration::ZERO);
        let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
        let before = draw(&controller, 120, 30);
        controller.dispatch(Action::Up, &geometry);
        assert_ne!(before, draw(&controller, 120, 30));
    }
}

#[test]
fn sidebar_wheel_reverses_immediately_at_the_content_limit() {
    let (_root, mut controller) = controller(true);
    controller.app.popup = false;
    controller.app.sidebar = true;
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 12));
    let area = geometry.sidebar.unwrap();
    let event = |kind| {
        Event::Mouse(MouseEvent {
            kind,
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        })
    };
    for _ in 0..1000 {
        controller.handle(event(MouseEventKind::ScrollDown), &geometry, Duration::ZERO);
    }
    let before = controller.app.metadata_scroll;
    assert!(before > 0 && before < 1000);
    controller.handle(event(MouseEventKind::ScrollUp), &geometry, Duration::ZERO);
    assert_eq!(controller.app.metadata_scroll, before - 1);
    assert_eq!(controller.app.active, 0);
}
