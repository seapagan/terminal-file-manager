use super::*;

#[test]
fn safety_hints_use_the_same_evidence_key_as_controls() {
    let (_root, mut app) = fixture();
    for index in 0..app.operations.history.len() {
        app.operations.cursor = index;
        let (terminal, _) = draw(&app, (120, 40), ColorLevel::NoColor);
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!text.contains("E evidence"));
        assert!(text.contains("e Evidence"));
    }
}

#[test]
fn narrow_unknown_warning_names_the_inert_retry_key_and_blocks_exit() {
    let (_root, mut app) = fixture();
    app.operations.cursor = app
        .operations
        .history
        .iter()
        .position(|job| job.state == JobState::Unknown)
        .unwrap();
    let (terminal, g) = draw(&app, (28, 40), ColorLevel::NoColor);
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("UNKNOWN: no r/exit"));
    assert!(!g.operation_enabled(KeyCode::Char('r')));
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    for code in [KeyCode::Char('r'), KeyCode::Char('q')] {
        c.handle(
            Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            &g,
            Duration::ZERO,
        );
        assert!(!c.quit);
        assert!(c.app.operations.outbox.iter().all(|intent| !matches!(
            intent,
            terminal_file_manager::operations::Intent::Submit(..)
        )));
    }
}
