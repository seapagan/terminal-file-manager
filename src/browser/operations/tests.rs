use super::*;
use crate::{
    app::App,
    input::Bindings,
    operations::{
        Decision, Event, JobState, Operation, Options, Outcome, Plan, Record, Stage, Update,
    },
};
use crossterm::event::{Event as InputEvent, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::{fs, path::Path, time::Instant};

fn fixture() -> (tempfile::TempDir, Controller, Queue, Plan) {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("target")).unwrap();
    fs::write(root.path().join("source"), "approved bytes").unwrap();
    let plan = Plan::paths(
        Operation::Copy,
        vec![root.path().join("source")],
        Some(root.path().join("target")),
    )
    .unwrap();
    let app = App::new(root.path().to_owned());
    let bindings = Bindings::new(&Default::default()).unwrap();
    let controller = Controller::new(app, bindings, Default::default());
    (root, controller, Queue::new().unwrap(), plan)
}

fn acknowledge(controller: &mut Controller, queue: &mut Queue, decision: Decision) -> u64 {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "operation acknowledgement timed out");
        let update = queue.next(remaining).expect("operation update");
        if let Update::Conflict { reply, .. } = &update {
            reply.send(decision.clone()).unwrap();
        }
        let finished = match &update {
            Update::Finished { id, .. } | Update::Preview { id, .. } => Some(*id),
            _ => None,
        };
        controller.complete_operation(update);
        if let Some(id) = finished {
            return id;
        }
    }
}

fn key(controller: &mut Controller, code: KeyCode) {
    let geometry = crate::ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.handle(
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        &geometry,
        Duration::ZERO,
    );
}

fn install(controller: &mut Controller, plan: Plan, record: Record) {
    controller.register_operation(100, plan, Options::default(), None);
    controller.complete_operation(Update::Finished { id: 100, record });
}

fn preview(queue: &mut Queue, plan: Plan) -> Plan {
    let id = queue.preview(plan).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "preview acknowledgement timed out");
        if let Update::Preview { id: finished, plan } = queue.next(remaining).unwrap() {
            assert_eq!(finished, id);
            return *plan;
        }
    }
}

#[test]
fn retry_copies_only_incomplete_operands_and_links_the_child_attempt() {
    let (root, mut controller, mut queue, mut plan) = fixture();
    let second = root.path().join("second");
    fs::write(&second, "second bytes").unwrap();
    plan.operands.push(crate::fs::inspect(&second));
    fs::write(root.path().join("target/second"), "old second").unwrap();
    let options = Options { checksum: true };
    controller
        .app
        .operations
        .outbox
        .push(Intent::Submit(plan, options));
    submit(&mut controller, &mut queue);
    let parent = acknowledge(&mut controller, &mut queue, Decision::Skip);
    let original = &controller.app.operations.history[0];
    assert_eq!(original.record.as_ref().unwrap().completed_files(), 1);
    assert!(original.can_retry());
    fs::write(
        root.path().join("target/source"),
        "preserve completed destination",
    )
    .unwrap();
    controller.app.operations.outbox.push(Intent::Retry(parent));
    submit(&mut controller, &mut queue);
    let child = controller.app.operations.history.last().unwrap();
    assert_ne!(child.id, parent);
    assert_eq!(child.retry_of, Some(parent));
    assert!(child.options.checksum);
    assert_eq!(child.plan.operands.len(), 1);
    assert_eq!(child.plan.operands[0].path, second);
    acknowledge(&mut controller, &mut queue, Decision::Replace);
    let child = controller.app.operations.history.last().unwrap();
    assert_eq!(child.state, JobState::Completed);
    assert!(child.record.as_ref().unwrap().entries[0].checksum_verified);
    assert_eq!(
        fs::read(root.path().join("target/second")).unwrap(),
        b"second bytes"
    );
    assert_eq!(
        fs::read(root.path().join("target/source")).unwrap(),
        b"preserve completed destination"
    );
}

#[test]
fn retry_rejects_a_replacement_at_the_frozen_source_path() {
    let (root, mut controller, mut queue, plan) = fixture();
    fs::write(root.path().join("target/source"), "destination retained").unwrap();
    controller
        .app
        .operations
        .outbox
        .push(Intent::Submit(plan, Options::default()));
    submit(&mut controller, &mut queue);
    let parent = acknowledge(&mut controller, &mut queue, Decision::Skip);
    fs::rename(
        root.path().join("source"),
        root.path().join("held-original"),
    )
    .unwrap();
    fs::write(root.path().join("source"), "unapproved replacement").unwrap();
    controller.app.operations.outbox.push(Intent::Retry(parent));
    submit(&mut controller, &mut queue);
    acknowledge(&mut controller, &mut queue, Decision::Replace);
    let child = controller.app.operations.history.last().unwrap();
    assert_eq!(child.retry_of, Some(parent));
    assert_eq!(
        child.record.as_ref().unwrap().entries[0].outcome,
        Outcome::Changed
    );
    assert_eq!(
        fs::read(root.path().join("source")).unwrap(),
        b"unapproved replacement"
    );
    assert_eq!(
        fs::read(root.path().join("held-original")).unwrap(),
        b"approved bytes"
    );
    assert_eq!(
        fs::read(root.path().join("target/source")).unwrap(),
        b"destination retained"
    );
}

#[test]
fn destructive_retry_requires_a_fresh_preview_and_enter_still_cancels() {
    for operation in [Operation::Delete, Operation::Trash] {
        let (root, mut controller, mut queue, mut plan) = fixture();
        plan.operation = operation;
        plan.destination = None;
        let plan = preview(&mut queue, plan);
        let record =
            crate::operations::execute(&plan, Options::default(), &true.into(), |_| Decision::Skip);
        assert_eq!(record.entries[0].outcome, Outcome::Cancelled);
        install(&mut controller, plan, record);
        controller.app.operations.outbox.push(Intent::Retry(100));
        submit(&mut controller, &mut queue);
        let Some(Dialog::Operation(dialog)) = &controller.app.operations.dialog else {
            panic!("destructive retry must open confirmation");
        };
        let preview_id = dialog.preview_id.unwrap();
        assert!(!dialog.ready);
        assert!(dialog.plan.preview.is_none());
        assert_eq!(dialog.retry_of, Some(100));
        assert_eq!(
            acknowledge(&mut controller, &mut queue, Decision::Skip),
            preview_id
        );
        let Some(Dialog::Operation(dialog)) = &controller.app.operations.dialog else {
            panic!("fresh preview must retain confirmation");
        };
        assert!(dialog.ready && dialog.plan.preview.is_some());
        assert_eq!(controller.app.operations.history.len(), 1);
        key(&mut controller, KeyCode::Enter);
        submit(&mut controller, &mut queue);
        assert!(controller.app.operations.dialog.is_none());
        assert_eq!(controller.app.operations.history.len(), 1);
        assert_eq!(
            fs::read(root.path().join("source")).unwrap(),
            b"approved bytes"
        );
    }
}

fn uncertain_stage(plan: &Plan, root: &Path) -> (Record, std::path::PathBuf) {
    let mut payload = None;
    let record = crate::operations::execute(plan, Options::default(), &false.into(), |event| {
        match event {
            Event::OwnedStage { payload: entry, .. } => payload = Some(entry.path),
            Event::Progress {
                stage: Stage::Verified,
                ..
            } => {
                let path = payload.as_ref().unwrap();
                fs::rename(path, root.join("held-stage")).unwrap();
                fs::write(path, "external stage replacement").unwrap();
            }
            _ => {}
        }
        Decision::Skip
    });
    assert!(record.reconcilable());
    (record, payload.unwrap())
}

#[test]
fn reconciliation_preserves_evidence_when_the_queue_does_not_own_the_claims() {
    let (root, mut controller, mut queue, plan) = fixture();
    let (record, stage) = uncertain_stage(&plan, root.path());
    install(&mut controller, plan, record);
    controller
        .app
        .operations
        .outbox
        .push(Intent::Reconcile(100));
    submit(&mut controller, &mut queue);
    assert_eq!(controller.app.operations.history[0].state, JobState::Queued);
    assert_eq!(
        acknowledge(&mut controller, &mut queue, Decision::Skip),
        100
    );
    let job = &controller.app.operations.history[0];
    assert_eq!(job.state, JobState::Unknown);
    assert_eq!(fs::read(&stage).unwrap(), b"external stage replacement");
    fs::rename(&stage, root.path().join("external-preserved")).unwrap();
    fs::rename(root.path().join("held-stage"), &stage).unwrap();
    controller
        .app
        .operations
        .outbox
        .push(Intent::Reconcile(100));
    submit(&mut controller, &mut queue);
    assert_eq!(
        acknowledge(&mut controller, &mut queue, Decision::Skip),
        100
    );
    assert!(
        controller.app.operations.history[0]
            .record
            .as_ref()
            .unwrap()
            .unresolved()
    );
    assert_eq!(fs::read(&stage).unwrap(), b"approved bytes");
    assert_eq!(
        fs::read(root.path().join("source")).unwrap(),
        b"approved bytes"
    );
    assert_eq!(
        fs::read(root.path().join("external-preserved")).unwrap(),
        b"external stage replacement"
    );
    assert!(!root.path().join("target/source").exists());
    assert_eq!(controller.app.operations.history.len(), 1);
}

#[test]
fn terminal_or_active_jobs_cannot_bypass_retry_and_reconciliation_guards() {
    let (root, mut controller, mut queue, plan) = fixture();
    let cancelled =
        crate::operations::execute(&plan, Options::default(), &true.into(), |_| Decision::Skip);
    for outcome in [
        None,
        Some(Outcome::Completed),
        Some(Outcome::SafetyIncident),
        Some(Outcome::OutcomeUnknown),
    ] {
        controller.app.operations.history.clear();
        controller.register_operation(100, plan.clone(), Options::default(), None);
        if let Some(outcome) = outcome {
            let mut record = cancelled.clone();
            record.entries[0].outcome = outcome;
            controller.complete_operation(Update::Finished { id: 100, record });
        }
        assert!(retry(&mut controller, &mut queue, 100).is_err());
        assert!(reconcile(&mut controller, &mut queue, 100).is_err());
        assert!(controller.app.operations.dialog.is_none());
        assert_eq!(controller.app.operations.history.len(), 1);
        assert!(queue.next(Duration::ZERO).is_none());
    }
    assert!(retry(&mut controller, &mut queue, 999).is_err());
    assert!(reconcile(&mut controller, &mut queue, 999).is_err());
    assert_eq!(
        fs::read(root.path().join("source")).unwrap(),
        b"approved bytes"
    );
    assert!(!root.path().join("target/source").exists());
}

fn partial_delete(controller: &mut Controller, queue: &mut Queue, root: &Path) {
    let pending = root.join("pending");
    fs::create_dir(&pending).unwrap();
    fs::write(pending.join("child"), "unfinished bytes").unwrap();
    let plan = Plan::paths(
        Operation::Delete,
        vec![root.join("source"), pending.clone()],
        None,
    )
    .unwrap();
    let plan = preview(queue, plan);
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let record = crate::operations::execute(&plan, Options::default(), &cancel, |event| {
        if let Event::Progress {
            source,
            stage: Stage::Removing,
            ..
        } = event
            && source.starts_with(&pending)
        {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Decision::Skip
    });
    assert_eq!(record.completed_files(), 1);
    assert!(!root.join("source").exists());
    assert_eq!(
        fs::read(pending.join("child")).unwrap(),
        b"unfinished bytes"
    );
    install(controller, plan, record);
    assert!(controller.app.operations.history[0].can_retry());
    fs::write(root.join("source"), "new occupant of completed path").unwrap();
}

fn open_delete_retry(controller: &mut Controller, queue: &mut Queue) -> u64 {
    key(controller, KeyCode::F(9));
    key(controller, KeyCode::Char('r'));
    assert!(matches!(
        controller.app.operations.outbox.as_slice(),
        [Intent::Retry(100)]
    ));
    submit(controller, queue);
    let Some(Dialog::Operation(dialog)) = &controller.app.operations.dialog else {
        panic!("retry must open a separate confirmation");
    };
    assert!(!dialog.ready);
    assert!(dialog.plan.preview.is_none());
    assert_eq!(dialog.retry_of, Some(100));
    let id = dialog.preview_id.unwrap();
    key(controller, KeyCode::Char('D'));
    assert!(controller.app.operations.outbox.is_empty());
    assert_eq!(acknowledge(controller, queue, Decision::Skip), id);
    id
}

#[test]
fn affirmative_delete_retry_counts_then_removes_only_unfinished_operands() {
    let (root, mut controller, mut queue, _) = fixture();
    partial_delete(&mut controller, &mut queue, root.path());
    let preview_id = open_delete_retry(&mut controller, &mut queue);
    let Some(Dialog::Operation(dialog)) = &controller.app.operations.dialog else {
        panic!("preview must await deliberate approval");
    };
    assert!(dialog.ready);
    let counts = dialog.plan.preview.as_ref().unwrap();
    assert_eq!((counts.files, counts.directories, counts.bytes), (1, 1, 16));
    assert_eq!(controller.app.operations.history.len(), 1);
    assert_eq!(
        fs::read(root.path().join("pending/child")).unwrap(),
        b"unfinished bytes"
    );
    key(&mut controller, KeyCode::Char('D'));
    assert!(matches!(
        controller.app.operations.outbox.as_slice(),
        [Intent::SubmitRetry(_, _, 100)]
    ));
    submit(&mut controller, &mut queue);
    let child_id = acknowledge(&mut controller, &mut queue, Decision::Skip);
    assert_ne!(child_id, preview_id);
    let child = controller.app.operations.history.last().unwrap();
    assert_eq!(child.retry_of, Some(100));
    assert_eq!(child.state, JobState::Completed);
    let record = child.record.as_ref().unwrap();
    assert_eq!(
        (record.completed_files(), record.completed_directories()),
        (1, 1)
    );
    assert!(
        record
            .entries
            .iter()
            .all(|entry| entry.source.starts_with(root.path().join("pending")))
    );
    assert!(!root.path().join("pending").exists());
    assert_eq!(
        fs::read(root.path().join("source")).unwrap(),
        b"new occupant of completed path"
    );
    let notice = controller.app.operations.notices.back().unwrap();
    assert_eq!(notice.severity, crate::operations::NoticeSeverity::Success);
    assert_eq!(
        notice.text,
        format!("Job {child_id}: 1 file permanently deleted, 1 directory permanently deleted")
    );
}

#[test]
fn approved_delete_retry_refuses_a_descendant_replaced_after_its_fresh_preview() {
    let (root, mut controller, mut queue, _) = fixture();
    partial_delete(&mut controller, &mut queue, root.path());
    open_delete_retry(&mut controller, &mut queue);
    let child = root.path().join("pending/child");
    fs::rename(&child, root.path().join("held-original")).unwrap();
    fs::write(&child, "foreign replacement").unwrap();
    key(&mut controller, KeyCode::Char('D'));
    submit(&mut controller, &mut queue);
    acknowledge(&mut controller, &mut queue, Decision::Skip);
    let job = controller.app.operations.history.last().unwrap();
    assert_eq!(job.retry_of, Some(100));
    assert_eq!(job.state, JobState::Failed);
    let record = job.record.as_ref().unwrap();
    assert_eq!(record.completed_files(), 0);
    assert_eq!(record.entries[0].outcome, Outcome::Changed);
    assert_eq!(fs::read(child).unwrap(), b"foreign replacement");
    assert_eq!(
        fs::read(root.path().join("held-original")).unwrap(),
        b"unfinished bytes"
    );
    assert_eq!(
        fs::read(root.path().join("source")).unwrap(),
        b"new occupant of completed path"
    );
}

#[test]
fn delete_retry_cannot_preview_or_authorize_a_replaced_original_root() {
    let (root, mut controller, mut queue, _) = fixture();
    partial_delete(&mut controller, &mut queue, root.path());
    fs::rename(root.path().join("pending"), root.path().join("held-root")).unwrap();
    fs::create_dir(root.path().join("pending")).unwrap();
    fs::write(root.path().join("pending/foreign"), "preserve").unwrap();
    open_delete_retry(&mut controller, &mut queue);
    let Some(Dialog::Operation(dialog)) = &controller.app.operations.dialog else {
        panic!("failed preview retains its disabled confirmation");
    };
    assert!(!dialog.ready);
    assert!(dialog.error.is_some());
    key(&mut controller, KeyCode::Char('D'));
    submit(&mut controller, &mut queue);
    assert_eq!(controller.app.operations.history.len(), 1);
    key(&mut controller, KeyCode::Enter);
    submit(&mut controller, &mut queue);
    assert!(controller.app.operations.dialog.is_none());
    assert_eq!(
        fs::read(root.path().join("pending/foreign")).unwrap(),
        b"preserve"
    );
    assert_eq!(
        fs::read(root.path().join("held-root/child")).unwrap(),
        b"unfinished bytes"
    );
}

#[path = "reconciliation_summary_tests.rs"]
mod reconciliation_summary_tests;
