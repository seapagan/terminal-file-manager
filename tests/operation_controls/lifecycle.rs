use super::*;

#[test]
fn cancel_then_exit_waits_for_acknowledgement_and_return_abandons_exit_intent() {
    let (_, mut controller) = controller();
    running(&mut controller);
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::SaveQuit, &geometry);
    assert!(!controller.quit);
    key(&mut controller, KeyCode::Enter);
    assert!(controller.app.operations.exit_snapshot.is_none());
    assert!(!controller.save_session);
    controller.dispatch(Action::Quit, &geometry);
    key(&mut controller, KeyCode::Char('c'));
    assert!(matches!(
        controller.app.operations.outbox.last(),
        Some(Intent::Cancel(1))
    ));
    assert!(!controller.quit);
    finish(&mut controller, Outcome::Cancelled);
    assert!(controller.quit);
    assert!(!controller.save_session);
}

#[test]
fn save_quit_freezes_navigation_until_safe_wait_finishes() {
    let (root, mut controller) = controller();
    running(&mut controller);
    let before = controller.app.panes[0].path.clone();
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::SaveQuit, &geometry);
    key(&mut controller, KeyCode::Char('w'));
    assert!(!controller.quit);
    controller.app.panes[0].path = root.path().join("later-navigation");
    finish(&mut controller, Outcome::Completed);
    assert!(controller.quit && controller.save_session);
    let snapshot = controller.app.operations.exit_snapshot.as_ref().unwrap();
    assert_eq!(snapshot.locations().unwrap()[0], before);
    let store = terminal_file_manager::session::SessionStore {
        path: root.path().join("saved.toml"),
    };
    store.save_snapshot(snapshot).unwrap();
    assert_eq!(store.load().unwrap().locations().unwrap()[0], before);
}

#[test]
fn an_unknown_outcome_blocks_retry_and_safe_exit_without_freezing_the_manager() {
    let (_, mut controller) = controller();
    running(&mut controller);
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::Quit, &geometry);
    key(&mut controller, KeyCode::Char('c'));
    finish(&mut controller, Outcome::OutcomeUnknown);
    assert!(!controller.quit);
    assert!(controller.app.operations.waiting_exit.is_none());
    assert!(controller.app.status.contains("unknown"));
    let job = &controller.app.operations.history[0];
    assert!(job.plan.retry(job.record.as_ref().unwrap()).is_err());
}

#[test]
fn a_native_trash_safety_incident_blocks_retry_exit_and_reconciliation() {
    let (_, mut controller) = controller();
    running(&mut controller);
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::Quit, &geometry);
    key(&mut controller, KeyCode::Char('c'));
    finish(&mut controller, Outcome::SafetyIncident);
    assert!(!controller.quit);
    let job = &controller.app.operations.history[0];
    let record = job.record.as_ref().unwrap();
    assert!(record.unresolved());
    assert!(job.plan.retry(record).is_err());
    let reconciled =
        terminal_file_manager::operations::reconcile(&job.plan, record.clone(), &false.into());
    assert!(reconciled.unresolved());
    assert_eq!(reconciled.entries[0].outcome, Outcome::SafetyIncident);
}

#[test]
fn jobs_without_ownership_evidence_do_not_offer_a_reconciliation_attempt() {
    let (_, mut controller) = controller();
    running(&mut controller);
    finish(&mut controller, Outcome::OutcomeUnknown);
    controller.app.operations.show_history = true;
    key(&mut controller, KeyCode::Char('x'));
    assert!(controller.app.operations.outbox.is_empty());
    assert!(!controller.app.operations.history[0].can_reconcile());
    assert_eq!(controller.app.operations.history.len(), 1);
    assert!(!controller.quit);
}

#[test]
fn completed_job_elapsed_time_stops_at_worker_acknowledgement() {
    let (_, mut controller) = controller();
    running(&mut controller);
    controller.app.now += Duration::from_secs(7);
    finish(&mut controller, Outcome::Completed);
    controller.app.now += Duration::from_secs(100);
    controller.app.operations.show_history = true;
    controller.app.operations.jobs_evidence = true;
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(
                frame,
                &controller.app,
                &Theme::new(colored_text::ColorLevel::NoColor),
            );
        })
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("elapsed ~7s"), "{text}");
}
