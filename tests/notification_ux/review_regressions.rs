use super::*;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use terminal_file_manager::{
    input::Action,
    operations::{Dialog, Operation, Plan},
};

fn press(c: &mut Controller, code: KeyCode) {
    let (_, geometry) = draw(c, 80);
    c.handle(
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        &geometry,
        Duration::ZERO,
    );
}

fn linked_job_over_dialog() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("finished"), "preserved collision").unwrap();
    let mut c = controller();
    c.app = App::new(root.path().into());
    c.app.complete_listing(
        0,
        1,
        Ok(terminal_file_manager::fs::list(
            root.path(),
            &terminal_file_manager::fs::IgnoreContext::isolated(),
            &false.into(),
        )
        .unwrap()),
    );
    complete(&mut c, Plan::create_file(root.path().join("finished")));
    assert!(c.app.operations.history[0].can_retry());
    let (_, geometry) = draw(&c, 80);
    c.dispatch(Action::Mkdir, &geometry);
    assert!(matches!(
        c.app.operations.dialog,
        Some(Dialog::Operation(_))
    ));
    press(&mut c, KeyCode::Char('p'));
    let (_, geometry) = draw(&c, 80);
    let toast = &geometry.toasts[0];
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: toast.area.x,
            row: toast.area.y,
            modifiers: KeyModifiers::NONE,
        }),
        &geometry,
        Duration::ZERO,
    );
    press(&mut c, KeyCode::Enter);
    (root, c)
}

#[test]
fn linked_jobs_owns_render_and_input_then_returns_to_the_pending_dialog() {
    let (_root, mut c) = linked_job_over_dialog();
    let (buffer, geometry) = draw(&c, 80);
    assert!(!c.app.notifications.open);
    assert!(text(&buffer, buffer.area).contains("Jobs / progress / results"));
    assert!(geometry.operation_enabled(KeyCode::Char('e')));
    press(&mut c, KeyCode::Char('e'));
    press(&mut c, KeyCode::Char('r'));
    assert!(c.app.operations.outbox.is_empty());
    press(&mut c, KeyCode::Esc);
    let Some(Dialog::Operation(dialog)) = &c.app.operations.dialog else {
        panic!("pending dialog lost")
    };
    assert_eq!(dialog.plan.operation, Operation::Mkdir);
    assert_eq!(dialog.text, "p");
    assert!(dialog.edited);
    assert!(!c.quit);
}

#[test]
fn suspended_preview_receives_completion_and_failure_before_dialog_return() {
    use terminal_file_manager::operations::{Record, Update};
    for failed in [false, true] {
        let (_root, mut c) = linked_job_over_dialog();
        let (Dialog::Operation(dialog), scroll) =
            c.app.operations.suspended_dialog.as_deref_mut().unwrap()
        else {
            panic!("pending operation was not suspended")
        };
        dialog.preview_id = Some(42);
        dialog.ready = false;
        *scroll = 3;
        let plan = dialog.plan.clone();
        if failed {
            c.complete_operation(Update::Finished {
                id: 42,
                record: Record::default(),
            });
        } else {
            c.complete_operation(Update::Preview {
                id: 42,
                plan: Box::new(plan),
            });
        }
        press(&mut c, KeyCode::Esc);
        let Some(Dialog::Operation(dialog)) = &c.app.operations.dialog else {
            panic!("pending dialog lost")
        };
        assert_eq!(dialog.ready, !failed);
        assert_eq!(dialog.error.is_some(), failed);
        assert_eq!(dialog.preview_id, Some(42));
        assert_eq!(c.app.operations.scroll, 3);
    }
}

#[test]
fn interrupt_cancels_the_suspended_preview_through_existing_exit_handling() {
    use terminal_file_manager::operations::Intent;
    let (_root, mut c) = linked_job_over_dialog();
    let (Dialog::Operation(dialog), _) = c.app.operations.suspended_dialog.as_deref_mut().unwrap()
    else {
        panic!("pending operation missing")
    };
    dialog.preview_id = Some(42);
    let (_, geometry) = draw(&c, 80);
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        &geometry,
        Duration::ZERO,
    );
    assert!(matches!(
        c.app.operations.outbox.as_slice(),
        [Intent::Cancel(42)]
    ));
    assert!(c.app.operations.suspended_dialog.is_none());
    assert!(c.quit);
}

#[test]
fn current_error_keeps_priority_and_history_routing_after_background_recovery() {
    let mut c = controller();
    c.register_operation(
        7,
        Plan::mkdir("owned-background".into()),
        Default::default(),
        None,
    );
    c.app.notify(Severity::Error, "Current failure");
    c.app
        .notifications
        .notify("Watch recovered".into(), c.app.now, Severity::Info);
    c.app.notifications.advance(Duration::from_secs(60));
    let (buffer, geometry) = draw(&c, 20);
    assert!(text(&buffer, geometry.status).starts_with("Current failure"));
    assert_eq!(geometry.status_controls[0].1, Action::Notifications);
    c.app.status = "Ordinary context".into();
    let (buffer, geometry) = draw(&c, 20);
    assert!(text(&buffer, geometry.status).starts_with("Jobs"));
}

#[test]
fn linked_evidence_cannot_bypass_unknown_or_incident_exit_guards() {
    use terminal_file_manager::operations::{Outcome, Update};
    for outcome in [Outcome::OutcomeUnknown, Outcome::SafetyIncident] {
        let (_root, mut c) = linked_job_over_dialog();
        let mut record = c.app.operations.history[0].record.clone().unwrap();
        record.entries[0].outcome = outcome;
        c.complete_operation(Update::Finished { id: 1, record });
        let (_, geometry) = draw(&c, 80);
        c.handle(
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            &geometry,
            Duration::ZERO,
        );
        assert!(!c.quit);
        assert!(c.app.operations.suspended_dialog.is_none());
        assert_eq!(
            c.app.operations.history[0].record.as_ref().unwrap().entries[0].outcome,
            outcome
        );
        assert!(matches!(
            c.app.operations.dialog,
            Some(Dialog::Exit { .. } | Dialog::IncidentExit { .. })
        ));
    }
}
