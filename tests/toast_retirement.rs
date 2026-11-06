use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{app::App, color::Theme, notifications::Severity, ui};

#[test]
fn rendering_a_tiny_frame_does_not_mutate_pending_presentations() {
    let mut app = App::new("owned-notifications".into());
    app.notify(Severity::Info, "Retained event");
    let before = app.notifications.visible();
    let mut terminal = Terminal::new(TestBackend::new(10, 4)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, &app, &Theme::new(colored_text::ColorLevel::NoColor));
        })
        .unwrap();
    assert_eq!(app.notifications.visible(), before);
    assert_eq!(app.notifications.history.len(), 1);
}

#[test]
fn tiny_terminal_update_retires_toasts_without_resurrection_or_losing_history() {
    for area in [Rect::new(0, 0, 10, 24), Rect::new(0, 0, 80, 4)] {
        let mut app = App::new("owned-notifications".into());
        app.notify(Severity::Error, "Retained failure");
        ui::prepare_notifications(&mut app, area);
        assert!(app.notifications.visible().is_empty());
        ui::prepare_notifications(&mut app, Rect::new(0, 0, 120, 60));
        assert!(app.notifications.visible().is_empty());
        assert_eq!(app.notifications.history.len(), 1);
    }
}
