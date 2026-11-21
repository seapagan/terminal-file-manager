use super::*;
use crate::{
    app::{PatternControl, PatternOperation},
    input::Action,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

fn draw(terminal: &mut Terminal<TestBackend>, app: &App) -> ui::Geometry {
    let mut geometry = None;
    terminal
        .draw(|frame| {
            geometry = Some(ui::render(
                frame,
                app,
                &Theme::new(colored_text::ColorLevel::NoColor),
            ));
        })
        .unwrap();
    geometry.unwrap()
}

fn mark_enabled(geometry: &ui::Geometry) -> bool {
    let targets = geometry.pattern.as_ref().unwrap().targets();
    targets.contains(&PatternControl::Apply(PatternOperation::Mark))
}

#[test]
fn unchanged_refresh_reenables_rendered_pattern_apply_without_an_input_event() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = crate::browser::tests::counting_controller(root.path());
    let mut pool = Pool::new(crate::fs::IgnoreContext::isolated()).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let geometry = draw(&mut terminal, &controller.app);
    controller.dispatch(Action::PatternSelection, &geometry);
    controller.app.pattern.as_mut().unwrap().append("direct");
    let geometry = draw(&mut terminal, &controller.app);
    assert!(mark_enabled(&geometry));
    let installs = controller.app.panes[0].listing_installs;
    let request = controller.app.request_reconciliation(0).unwrap();
    controller.handle(Event::Paste("ory".into()), &geometry, Duration::ZERO);
    let geometry = draw(&mut terminal, &controller.app);
    assert!(!mark_enabled(&geometry));
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .contains("Listing unavailable or refreshing")
    );
    pool.submit(request);
    // This helper requires collect to request a redraw, as the runtime does.
    crate::browser::tests::collect_until_idle(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].listing_installs, installs);
    let geometry = draw(&mut terminal, &controller.app);
    assert!(mark_enabled(&geometry));
    controller.handle(
        Event::Key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT)),
        &geometry,
        Duration::ZERO,
    );
    assert!(controller.app.pattern.is_none());
    assert_eq!(controller.app.panes[0].selection_counts().visible, 1);
}

#[test]
fn unchanged_refresh_without_pattern_selection_does_not_request_redraw() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = crate::browser::tests::counting_controller(root.path());
    let mut pool = Pool::new(crate::fs::IgnoreContext::isolated()).unwrap();
    pool.submit(controller.app.request_reconciliation(0).unwrap());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while pool.active_count() != 0 {
        assert!(!collect(&mut controller, &mut pool));
        assert!(std::time::Instant::now() < deadline, "listing timed out");
        std::thread::yield_now();
    }
    assert!(!controller.app.panes[0].listing);
}
