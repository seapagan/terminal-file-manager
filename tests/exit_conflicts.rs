use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use std::{fs, sync::mpsc, time::Duration};
use terminal_file_manager::{
    app::App,
    input::{Bindings, Controller},
    operations::{Decision, Intent, JobState, Operation, Options, Plan, Queue, Update, execute},
    ui::{self, OperationTarget},
};

fn fixture() -> (tempfile::TempDir, Controller, Plan) {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), "incoming").unwrap();
    fs::create_dir(root.path().join("target")).unwrap();
    fs::write(root.path().join("target/source"), "existing").unwrap();
    let plan = Plan::paths(
        Operation::Copy,
        vec![root.path().join("source")],
        Some(root.path().join("target")),
    )
    .unwrap();
    let app = App::new(root.path().into());
    let controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    (root, controller, plan)
}

fn event(controller: &mut Controller, event: Event) {
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    assert!(
        controller
            .handle(event, &geometry, Duration::ZERO)
            .is_empty()
    );
}

fn key(controller: &mut Controller, code: KeyCode) {
    event(
        controller,
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
    );
}

fn wait(controller: &mut Controller, save: bool, cancel: bool) {
    key(controller, KeyCode::Char(if save { 'Q' } else { 'q' }));
    key(controller, KeyCode::Char(if cancel { 'c' } else { 'w' }));
    assert_eq!(controller.app.operations.waiting_exit, Some(save));
    assert!(!controller.quit);
}

fn conflict(controller: &mut Controller, id: u64, plan: &Plan) -> mpsc::Receiver<Decision> {
    let (reply, response) = mpsc::sync_channel(1);
    controller.complete_operation(Update::Conflict {
        id,
        event: terminal_file_manager::operations::Event::Conflict {
            source: Box::new(plan.operands[0].clone()),
            destination: Box::new(terminal_file_manager::fs::inspect(
                &plan
                    .destination
                    .as_ref()
                    .unwrap()
                    .join(&plan.operands[0].name),
            )),
            newer_available: true,
        },
        reply,
    });
    response
}

#[test]
fn multiple_conflicts_keep_exit_pending_until_both_jobs_acknowledge() {
    let (_, mut controller, plan) = fixture();
    for id in [1, 2] {
        controller.register_operation(id, plan.clone(), Options::default(), None);
    }
    wait(&mut controller, true, false);
    let first = conflict(&mut controller, 1, &plan);
    let second = conflict(&mut controller, 2, &plan);
    key(&mut controller, KeyCode::Char('s'));
    assert!(matches!(first.try_recv().unwrap(), Decision::Skip));
    assert!(matches!(second.try_recv(), Err(mpsc::TryRecvError::Empty)));
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    controller.complete_operation(Update::Finished { id: 1, record });
    assert!(!controller.quit);
    assert_eq!(controller.app.operations.waiting_exit, Some(true));
    key(&mut controller, KeyCode::F(9));
    key(&mut controller, KeyCode::Up);
    for code in [KeyCode::Char('r'), KeyCode::Char('x')] {
        key(&mut controller, code);
    }
    assert!(controller.app.operations.outbox.is_empty());
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    assert!(
        !geometry
            .operation_controls
            .iter()
            .any(|control| control.target == OperationTarget::Key(KeyCode::Char('r')))
    );
    key(&mut controller, KeyCode::Down);
    key(&mut controller, KeyCode::Enter);
    key(&mut controller, KeyCode::Esc);
    assert!(matches!(second.try_recv().unwrap(), Decision::Cancel));
    assert!(!controller.quit);
    let record = execute(&plan, Options::default(), &true.into(), |_| Decision::Skip);
    controller.complete_operation(Update::Finished { id: 2, record });
    assert!(controller.quit && controller.save_session);
}

fn assert_reply(code: KeyCode, decision: &Decision) {
    assert!(matches!(
        (code, decision),
        (KeyCode::Char('s') | KeyCode::Enter, Decision::Skip)
            | (KeyCode::Char('r'), Decision::Replace)
            | (KeyCode::Char('k'), Decision::KeepBoth)
            | (KeyCode::Esc, Decision::Cancel)
    ));
}

fn reply_then_ack(save: bool, cancel: bool, choice: KeyCode) {
    let (root, mut controller, plan) = fixture();
    controller.register_operation(1, plan.clone(), Options::default(), None);
    wait(&mut controller, save, cancel);
    let response = conflict(&mut controller, 1, &plan);
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        event(
            &mut controller,
            Event::Key(KeyEvent::new_with_kind(choice, KeyModifiers::NONE, kind)),
        );
        assert!(matches!(
            response.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert_eq!(controller.app.operations.waiting_exit, Some(save));
    }
    key(&mut controller, choice);
    let decision = response
        .try_recv()
        .expect("Conflict response must reach the worker immediately");
    assert_reply(choice, &decision);
    assert!(matches!(
        response.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
    assert_eq!(controller.app.operations.waiting_exit, Some(save));
    assert!(!controller.quit);
    let record = execute(&plan, Options::default(), &cancel.into(), |_| {
        decision.clone()
    });
    assert_eq!(fs::read(root.path().join("source")).unwrap(), b"incoming");
    let replaced = !cancel && matches!(decision, Decision::Replace);
    assert_eq!(
        fs::read(root.path().join("target/source")).unwrap(),
        if replaced {
            b"incoming".as_slice()
        } else {
            b"existing".as_slice()
        }
    );
    controller.complete_operation(Update::Finished { id: 1, record });
    assert!(controller.quit);
    assert_eq!(controller.save_session, save);
    assert_eq!(controller.app.operations.exit_snapshot.is_some(), save);
}

#[test]
fn waiting_exit_routes_conflict_choices_once_and_keeps_q_or_q_save_intent_until_ack() {
    for save in [false, true] {
        for cancel in [false, true] {
            for choice in [
                KeyCode::Char('s'),
                KeyCode::Char('r'),
                KeyCode::Char('k'),
                KeyCode::Enter,
                KeyCode::Esc,
            ] {
                reply_then_ack(save, cancel, choice);
            }
        }
    }
}

#[test]
fn waiting_exit_mouse_conflict_hit_regions_reply_without_allowing_pane_actions() {
    let (_, mut controller, plan) = fixture();
    controller.register_operation(1, plan.clone(), Options::default(), None);
    wait(&mut controller, false, false);
    let response = conflict(&mut controller, 1, &plan);
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    let area = geometry
        .operation_controls
        .iter()
        .find(|control| control.target == OperationTarget::Key(KeyCode::Char('r')))
        .unwrap()
        .area;
    assert!(
        !geometry
            .operation_controls
            .iter()
            .any(|control| control.area.contains((area.right(), area.y).into()))
    );
    for column in [area.right(), area.x] {
        event(
            &mut controller,
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row: area.y,
                modifiers: KeyModifiers::NONE,
            }),
        );
        if column == area.right() {
            assert!(matches!(
                response.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
        }
    }
    assert!(matches!(response.try_recv().unwrap(), Decision::Replace));
    for code in [
        KeyCode::Down,
        KeyCode::Tab,
        KeyCode::F(5),
        KeyCode::Char('q'),
        KeyCode::Char('Q'),
    ] {
        key(&mut controller, code);
    }
    assert!(controller.app.operations.outbox.is_empty());
    assert_eq!(controller.app.active, 0);
    assert_eq!(controller.app.operations.waiting_exit, Some(false));
    assert!(!controller.quit);
}

#[test]
fn conflict_escape_cancels_that_job_then_a_second_escape_abandons_pending_exit() {
    let (_, mut controller, plan) = fixture();
    controller.register_operation(1, plan.clone(), Options::default(), None);
    wait(&mut controller, true, false);
    let response = conflict(&mut controller, 1, &plan);
    key(&mut controller, KeyCode::Esc);
    assert!(matches!(response.try_recv().unwrap(), Decision::Cancel));
    assert_eq!(controller.app.operations.waiting_exit, Some(true));
    key(&mut controller, KeyCode::Esc);
    assert!(controller.app.operations.waiting_exit.is_none());
    assert!(controller.app.operations.exit_snapshot.is_none());
    let record = execute(&plan, Options::default(), &true.into(), |_| Decision::Skip);
    controller.complete_operation(Update::Finished { id: 1, record });
    assert!(!controller.quit && !controller.save_session);
}

#[test]
fn queued_worker_conflict_after_waiting_save_exit_requires_finished_acknowledgement() {
    let (root, mut controller, plan) = fixture();
    let mut queue = Queue::new().unwrap();
    let id = queue.submit(plan.clone(), Options::default()).unwrap();
    controller.register_operation(id, plan, Options::default(), None);
    wait(&mut controller, true, false);
    loop {
        let update = queue
            .next(Duration::from_secs(10))
            .expect("Worker must report its pending conflict");
        let pending = matches!(update, Update::Conflict { .. });
        controller.complete_operation(update);
        if pending {
            break;
        }
    }
    key(&mut controller, KeyCode::Char('r'));
    assert_eq!(controller.app.operations.waiting_exit, Some(true));
    assert!(!controller.quit);
    loop {
        let update = queue
            .next(Duration::from_secs(10))
            .expect("Answered worker must acknowledge completion");
        let finished = matches!(update, Update::Finished { .. });
        assert!(!controller.quit);
        controller.complete_operation(update);
        if finished {
            break;
        }
    }
    assert_eq!(
        fs::read(root.path().join("target/source")).unwrap(),
        b"incoming"
    );
    assert!(controller.quit && controller.save_session);
    assert!(!controller.app.operations.active());
}

#[test]
fn safely_cancelled_worker_ignores_a_stale_conflict_choice_and_exits_only_after_ack() {
    let (root, mut controller, plan) = fixture();
    let mut queue = Queue::new().unwrap();
    let id = queue.submit(plan.clone(), Options::default()).unwrap();
    controller.register_operation(id, plan, Options::default(), None);
    let pending = loop {
        let update = queue
            .next(Duration::from_secs(10))
            .expect("Worker conflict");
        if matches!(update, Update::Conflict { .. }) {
            break update;
        }
        controller.complete_operation(update);
    };
    wait(&mut controller, false, true);
    assert!(
        matches!(controller.app.operations.outbox.pop(), Some(Intent::Cancel(job)) if job == id)
    );
    queue.cancel(id);
    controller.complete_operation(pending);
    key(&mut controller, KeyCode::Char('r'));
    assert!(!controller.quit);
    loop {
        let update = queue
            .next(Duration::from_secs(10))
            .expect("Cancellation acknowledgement");
        let finished = matches!(update, Update::Finished { .. });
        assert!(!controller.quit);
        controller.complete_operation(update);
        if finished {
            break;
        }
    }
    assert!(controller.quit && !controller.save_session);
    assert_eq!(
        controller.app.operations.history[0].state,
        JobState::Cancelled
    );
    assert_eq!(
        fs::read(root.path().join("target/source")).unwrap(),
        b"existing"
    );
    assert_eq!(fs::read(root.path().join("source")).unwrap(), b"incoming");
}

#[test]
fn diagnostics_cannot_bypass_pending_exit_or_steal_hidden_conflict_input() {
    let (_, mut controller, plan) = fixture();
    controller.app.dual = true;
    controller.register_operation(1, plan.clone(), Options::default(), None);
    wait(&mut controller, true, false);
    let response = conflict(&mut controller, 1, &plan);
    key(&mut controller, KeyCode::F(12));
    assert!(controller.app.diagnostics);
    for code in [
        KeyCode::Tab,
        KeyCode::Down,
        KeyCode::F(5),
        KeyCode::Char('r'),
    ] {
        key(&mut controller, code);
    }
    assert_eq!(controller.app.active, 0);
    assert!(controller.app.operations.dialog.is_none());
    assert!(controller.app.operations.outbox.is_empty());
    assert!(matches!(
        response.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    key(&mut controller, KeyCode::Esc);
    assert!(controller.app.operations.waiting_exit.is_none());
    assert!(controller.app.operations.exit_snapshot.is_none());
    assert!(controller.app.diagnostics);
    assert!(!controller.quit);
}

#[test]
fn pending_exit_return_button_abandons_snapshot_without_cancelling_the_job() {
    let (_, mut controller, plan) = fixture();
    controller.register_operation(1, plan, Options::default(), None);
    wait(&mut controller, true, false);
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    let area = geometry
        .operation_controls
        .iter()
        .find(|control| control.target == OperationTarget::Key(KeyCode::Esc))
        .unwrap()
        .area;
    event(
        &mut controller,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert!(controller.app.operations.waiting_exit.is_none());
    assert!(controller.app.operations.exit_snapshot.is_none());
    assert!(controller.app.operations.outbox.is_empty());
    assert!(controller.app.operations.active());
    assert!(!controller.quit);
}
