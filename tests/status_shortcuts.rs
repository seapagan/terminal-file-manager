use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Action, Bindings, Controller},
    ui,
};

fn controller() -> Controller {
    let keys = Bindings::new(
        &[
            ("Alt+m".into(), Action::Help),
            ("F11".into(), Action::Maximise),
            ("F9".into(), Action::Help),
            ("Alt+j".into(), Action::Help),
            ("F1".into(), Action::Jobs),
        ]
        .into(),
    )
    .unwrap();
    Controller::new(App::new("owned-status".into()), keys, Default::default())
}
fn draw(app: &App, width: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(ColorLevel::NoColor));
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
fn maximised_pane_status_uses_the_effective_restore_shortcut() {
    let mut c = controller();
    assert!(draw(&c.app, 160).contains("F1 for jobs"));
    c.app.dual = true;
    let g = ui::geometry(&c.app, Rect::new(0, 0, 160, 24));
    c.dispatch(Action::Maximise, &g);
    let text = draw(&c.app, 160);
    assert!(text.contains("F11 restores split"), "{text}");
    assert!(!text.contains("Alt+m restores split"));
}
#[test]
fn literal_status_and_notice_text_are_never_interpreted_as_shortcut_templates() {
    let mut c = controller();
    for text in [
        "Local files — ? for help, F9 / Alt+j for jobs",
        "Owned failure — F9 details",
    ] {
        c.app.status = text.into();
        assert!(draw(&c.app, 160).contains(text));
    }
    c.app.notifications.notify(
        "Literal — F9 details".into(),
        c.app.now,
        terminal_file_manager::operations::NoticeSeverity::Error,
    );
    let text = draw(&c.app, 160);
    assert!(
        text.contains("Error: Literal — F9 details — Alt+h details"),
        "{text}"
    );
}
#[test]
fn rejected_request_keeps_effective_details_route_when_clipped() {
    let mut c = controller();
    c.app.reject_operation("long reason ".repeat(50));
    c.app.notifications.dismiss_toasts();
    let text = draw(&c.app, 25);
    assert!(text.contains("F1 details"), "{text}");
    assert!(!text.contains("F9 details"));
}
