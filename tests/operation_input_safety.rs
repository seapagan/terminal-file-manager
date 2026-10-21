use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::{fs, time::Duration};
use terminal_file_manager::{
    app::App,
    fs::IgnoreContext,
    input::{Action, Bindings, Controller},
    operations::{Dialog, NoticeSeverity, Operation, Outcome, Plan, Record, Update},
    ui,
};

fn fixture() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("left")).unwrap();
    fs::create_dir(root.path().join("right")).unwrap();
    fs::write(root.path().join("left/source"), "preserved").unwrap();
    let mut app = App::new(root.path().join("left"));
    for (id, name) in [(0, "left"), (1, "right")] {
        let snapshot = terminal_file_manager::fs::list(
            &root.path().join(name),
            &IgnoreContext::default(),
            &false.into(),
        )
        .unwrap();
        app.complete_listing(id, 1, Ok(snapshot));
    }
    let controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    (root, controller)
}

fn event(controller: &mut Controller, event: Event) {
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.handle(event, &geometry, Duration::ZERO);
}

fn key(controller: &mut Controller, code: KeyCode, modifiers: KeyModifiers) {
    event(controller, Event::Key(KeyEvent::new(code, modifiers)));
}

fn open(controller: &mut Controller, action: Action) {
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(action, &geometry);
}

fn replace_field(controller: &mut Controller, text: &str) {
    key(controller, KeyCode::Char('a'), KeyModifiers::CONTROL);
    event(controller, Event::Paste(text.into()));
    key(controller, KeyCode::Enter, KeyModifiers::NONE);
}

#[test]
fn rename_rejects_empty_nul_and_parent_traversal_without_queueing_or_retargeting() {
    for text in ["", "../escape", "bad\0name"] {
        let (root, mut controller) = fixture();
        open(&mut controller, Action::Rename);
        replace_field(&mut controller, text);
        let Some(Dialog::Operation(dialog)) = &controller.app.operations.dialog else {
            panic!("invalid name must remain in the dialog");
        };
        assert!(dialog.error.is_some());
        assert_eq!(
            dialog.plan.operands[0].path,
            root.path().join("left/source")
        );
        assert!(controller.app.operations.outbox.is_empty());
        assert_eq!(
            fs::read(root.path().join("left/source")).unwrap(),
            b"preserved"
        );
        assert!(!root.path().join("escape").exists());
    }
}

#[test]
fn invalid_attributes_and_oversized_paste_never_submit_an_operation() {
    let (root, mut controller) = fixture();
    open(&mut controller, Action::Attributes);
    replace_field(&mut controller, "not a mode or boolean");
    assert!(
        matches!(&controller.app.operations.dialog, Some(Dialog::Operation(dialog)) if dialog.error.is_some())
    );
    assert!(controller.app.operations.outbox.is_empty());
    key(&mut controller, KeyCode::Esc, KeyModifiers::NONE);
    open(&mut controller, Action::Rename);
    let before = match &controller.app.operations.dialog {
        Some(Dialog::Operation(dialog)) => dialog.text.clone(),
        _ => panic!("rename dialog"),
    };
    event(&mut controller, Event::Paste("x".repeat(65_537)));
    assert!(
        matches!(&controller.app.operations.dialog, Some(Dialog::Operation(dialog)) if dialog.text == before)
    );
    assert!(controller.app.status.contains("rejected"));
    assert!(controller.app.operations.outbox.is_empty());
    assert_eq!(
        fs::read(root.path().join("left/source")).unwrap(),
        b"preserved"
    );
}

fn install_outcome(controller: &mut Controller, outcomes: &[Outcome]) {
    let plan = Plan::paths(
        Operation::Copy,
        vec![controller.app.panes[0].path.join("source")],
        Some(controller.app.panes[1].path.clone()),
    )
    .unwrap();
    let template =
        terminal_file_manager::operations::execute(&plan, Default::default(), &true.into(), |_| {
            terminal_file_manager::operations::Decision::Skip
        });
    let entries = outcomes
        .iter()
        .map(|outcome| {
            let mut entry = template.entries[0].clone();
            entry.outcome = *outcome;
            entry
        })
        .collect();
    controller.register_operation(1, plan, Default::default(), None);
    controller.complete_operation(Update::Finished {
        id: 1,
        record: Record { entries },
    });
}

#[test]
fn acknowledged_outcomes_have_matching_notice_severity_and_observed_counts() {
    use NoticeSeverity::{Error, Success, Warning};
    use Outcome::*;
    let cases = [
        (vec![Completed], Success, "1 file copied"),
        (vec![Skipped], Warning, "0 files copied; 1 skipped"),
        (
            vec![Completed, Skipped],
            Warning,
            "1 file copied; 1 skipped",
        ),
        (vec![Changed], Error, "1 changed"),
        (vec![Failed], Error, "1 failed"),
        (vec![Cancelled], Warning, "1 cancelled"),
        (vec![CopiedSourceRetained], Warning, "1 source retained"),
        (vec![OutcomeUnknown], Error, "1 unknown"),
    ];
    for (outcomes, severity, text) in cases {
        let (_root, mut controller) = fixture();
        install_outcome(&mut controller, &outcomes);
        let notice = controller.app.operations.notices.back().unwrap();
        assert_eq!(notice.severity, severity);
        assert!(notice.text.contains(text), "{}", notice.text);
        assert!(controller.app.status.contains(text));
        assert_eq!(
            controller.app.operations.history[0]
                .record
                .as_ref()
                .unwrap()
                .entries
                .len(),
            outcomes.len()
        );
    }
}

#[test]
fn a_critical_acknowledgement_never_becomes_an_expiring_completion_notice() {
    let (_root, mut controller) = fixture();
    install_outcome(&mut controller, &[Outcome::SafetyIncident]);
    assert!(controller.app.operations.notices.is_empty());
    assert!(controller.app.operations.incidents());
    assert!(controller.app.status.contains("CRITICAL SAFETY INCIDENT"));
}
