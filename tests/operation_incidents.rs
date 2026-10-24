use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::{fs, time::Duration};
use terminal_file_manager::{
    app::App,
    input::{Action, Bindings, Controller},
    operations::{EntryResult, Operation, Outcome, Plan, Record, Update},
    ui,
};

fn fixture() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("owned"), "preserved").unwrap();
    let app = App::new(root.path().to_owned());
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    register(&mut controller, 1);
    (root, controller)
}

fn register(controller: &mut Controller, id: u64) {
    let plan = Plan::paths(
        Operation::Trash,
        vec![controller.app.panes[0].path.join("owned")],
        None,
    )
    .unwrap();
    controller.register_operation(id, plan, Default::default(), None);
}

fn finish(controller: &mut Controller, id: u64, outcome: Outcome) {
    let source = controller.app.operations.history[0].plan.operands[0].clone();
    controller.complete_operation(Update::Finished {
        id,
        record: Record {
            entries: vec![EntryResult {
                source: source.path,
                kind: source.kind,
                outcome,
                destination: None,
                recovery: None,
                trash: None,
                bytes: 0,
                checksum_verified: false,
                created_directory: false,
                detail: "controlled native acknowledgement; retained diagnostics".into(),
            }],
        },
    });
}

fn event(controller: &mut Controller, event: Event) {
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 100, 30));
    controller.handle(event, &geometry, Duration::ZERO);
}

fn key(controller: &mut Controller, code: KeyCode) {
    event(
        controller,
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
    );
}

fn request_exit(controller: &mut Controller, save: bool) {
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 100, 30));
    controller.dispatch(
        if save { Action::SaveQuit } else { Action::Quit },
        &geometry,
    );
}

fn acknowledge(controller: &mut Controller) {
    for c in "ACKNOWLEDGE".chars() {
        key(controller, KeyCode::Char(c));
    }
    key(controller, KeyCode::F(10));
}

#[test]
fn an_incident_can_exit_only_after_deliberate_confirmation_without_clearing_evidence() {
    for save in [false, true] {
        let (_root, mut controller) = fixture();
        finish(&mut controller, 1, Outcome::SafetyIncident);
        request_exit(&mut controller, save);
        event(&mut controller, Event::Paste("ACKNOWLEDGE".into()));
        key(&mut controller, KeyCode::F(10));
        assert!(!controller.quit);
        acknowledge(&mut controller);
        assert!(controller.quit);
        assert_eq!(controller.save_session, save);
        let record = controller.app.operations.history[0]
            .record
            .as_ref()
            .unwrap();
        assert_eq!(record.entries[0].outcome, Outcome::SafetyIncident);
        assert!(record.entries[0].detail.contains("retained diagnostics"));
    }
}

#[test]
fn ordinary_unknown_cannot_use_the_incident_exit_even_in_a_mixed_record() {
    let (_root, mut controller) = fixture();
    finish(&mut controller, 1, Outcome::SafetyIncident);
    register(&mut controller, 2);
    finish(&mut controller, 2, Outcome::OutcomeUnknown);
    request_exit(&mut controller, true);
    acknowledge(&mut controller);
    assert!(!controller.quit && !controller.save_session);
}

#[test]
fn active_workers_must_acknowledge_stop_before_incident_exit_is_available() {
    let (_root, mut controller) = fixture();
    finish(&mut controller, 1, Outcome::SafetyIncident);
    register(&mut controller, 2);
    request_exit(&mut controller, false);
    acknowledge(&mut controller);
    assert!(!controller.quit);
    key(&mut controller, KeyCode::Char('c'));
    assert!(matches!(
        controller.app.operations.outbox.last(),
        Some(terminal_file_manager::operations::Intent::Cancel(2))
    ));
    assert!(!controller.quit);
    finish(&mut controller, 2, Outcome::Cancelled);
    acknowledge(&mut controller);
    assert!(controller.quit);
    assert_eq!(
        controller.app.operations.history[0]
            .record
            .as_ref()
            .unwrap()
            .entries[0]
            .outcome,
        Outcome::SafetyIncident
    );
}

#[test]
fn accidental_keys_and_paste_cannot_acknowledge_an_incident() {
    let (_root, mut controller) = fixture();
    finish(&mut controller, 1, Outcome::SafetyIncident);
    for code in [
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Char('q'),
        KeyCode::Char('Q'),
        KeyCode::Char('D'),
    ] {
        request_exit(&mut controller, true);
        key(&mut controller, code);
        assert!(!controller.quit && !controller.save_session);
    }
}

#[test]
fn release_events_and_unrelated_clicks_never_confirm_or_retarget_the_incident() {
    let (_root, mut controller) = fixture();
    finish(&mut controller, 1, Outcome::SafetyIncident);
    request_exit(&mut controller, false);
    for c in "ACKNOWLEDGE".chars() {
        event(
            &mut controller,
            Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char(c),
                KeyModifiers::NONE,
                crossterm::event::KeyEventKind::Release,
            )),
        );
    }
    key(&mut controller, KeyCode::F(10));
    assert!(!controller.quit);
    let before = controller.app.panes[0].cursor.clone();
    event(
        &mut controller,
        Event::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert_eq!(controller.app.panes[0].cursor, before);
    assert!(!controller.quit);
}

#[test]
fn returning_abandons_session_save_but_retains_the_incident_and_unknown_is_strict() {
    let (_root, mut controller) = fixture();
    finish(&mut controller, 1, Outcome::SafetyIncident);
    request_exit(&mut controller, true);
    key(&mut controller, KeyCode::Enter);
    assert!(controller.app.operations.exit_snapshot.is_none());
    assert!(
        controller.app.operations.history[0]
            .record
            .as_ref()
            .unwrap()
            .incident()
    );
    let mut record = controller.app.operations.history[0].record.clone().unwrap();
    let mut unknown = record.entries[0].clone();
    unknown.outcome = Outcome::OutcomeUnknown;
    record.entries.push(unknown);
    controller.complete_operation(Update::Finished { id: 1, record });
    request_exit(&mut controller, false);
    acknowledge(&mut controller);
    assert!(!controller.quit && !controller.save_session);
}

#[test]
fn interrupting_a_pending_save_exit_requires_a_new_choice_before_worker_completion() {
    let (_root, mut controller) = fixture();
    request_exit(&mut controller, true);
    key(&mut controller, KeyCode::Char('w'));
    assert_eq!(controller.app.operations.waiting_exit, Some(true));
    event(
        &mut controller,
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
    );
    finish(&mut controller, 1, Outcome::Cancelled);
    assert!(!controller.quit && !controller.save_session);
    assert!(controller.app.operations.exit_snapshot.is_none());
}
