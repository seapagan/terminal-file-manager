use std::{
    fs,
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};
use terminal_file_manager::operations::{
    Decision, Event, Job, JobState, Operation, Options, Outcome, Plan, Record, Stage, execute,
};

fn fixture(operation: Operation) -> (tempfile::TempDir, Plan) {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("source")).unwrap();
    fs::create_dir(root.path().join("target")).unwrap();
    fs::write(root.path().join("source/child"), b"completed child").unwrap();
    let plan = Plan::paths(
        operation,
        vec![root.path().join("source")],
        Some(root.path().join("target")),
    )
    .unwrap();
    (root, plan)
}

fn job(plan: Plan, record: Record) -> Job {
    let mut job = Job {
        id: 1,
        plan,
        options: Options::default(),
        state: JobState::Running,
        record: None,
        progress: None,
        started: SystemTime::UNIX_EPOCH,
        finished: None,
        retry_of: None,
    };
    job.finish(record, SystemTime::UNIX_EPOCH);
    job
}

fn assert_partial(plan: &Plan, record: &Record) {
    assert_eq!(record.entries.len(), 2, "{record:?}");
    assert_eq!(record.entries[0].outcome, Outcome::Completed);
    let parent = &record.entries[1];
    assert_eq!(parent.outcome, Outcome::CopiedSourceRetained, "{record:?}");
    assert!(parent.recovery.is_some());
    assert!(!record.unresolved());
    let job = job(plan.clone(), record.clone());
    assert_eq!(job.state, JobState::Partial);
    assert!(job.can_retry());
    assert!(!job.can_reconcile());
    assert_eq!(
        terminal_file_manager::fs::inspect(&plan.operands[0].path)
            .identity
            .unwrap()
            .object,
        plan.operands[0].identity.as_ref().unwrap().object
    );
}

#[cfg(unix)]
#[test]
fn permission_finalization_failure_keeps_child_results_and_directory_evidence() {
    for operation in [Operation::Copy, Operation::Move] {
        let (root, mut plan) = fixture(operation);
        plan.operands[0].mode = None;
        let record = execute(&plan, Options { checksum: true }, &false.into(), |_| {
            Decision::Skip
        });
        assert_partial(&plan, &record);
        assert!(record.entries[1].detail.contains("permissions unavailable"));
        assert_eq!(
            fs::read(root.path().join("target/source/child")).unwrap(),
            b"completed child"
        );
        assert!(root.path().join("source").is_dir());
        assert_eq!(
            root.path().join("source/child").exists(),
            operation == Operation::Copy
        );
    }
}

#[test]
fn directory_remove_failure_never_discards_completed_children_or_retries_a_foreign_target() {
    let (root, plan) = fixture(Operation::Move);
    let source = root.path().join("source");
    let target = root.path().join("target/source");
    let mut injected = false;
    let record = execute(&plan, Options { checksum: true }, &false.into(), |event| {
        if matches!(event, Event::Progress { stage: Stage::Removing, source: ref path, .. } if path == &source)
        {
            assert!(!source.join("child").exists());
            fs::write(source.join("late arrival"), b"unrelated bytes").unwrap();
            injected = true;
        }
        Decision::Skip
    });
    assert!(injected);
    assert_partial(&plan, &record);
    assert_eq!(fs::read(target.join("child")).unwrap(), b"completed child");
    assert_eq!(
        fs::read(source.join("late arrival")).unwrap(),
        b"unrelated bytes"
    );
    let retry = plan.retry(&record).unwrap();
    fs::rename(&target, root.path().join("retained-target")).unwrap();
    fs::create_dir(&target).unwrap();
    let retried = execute(&retry, Options::default(), &false.into(), |_| {
        panic!("foreign target reached worker mutations")
    });
    assert_eq!(retried.entries[0].outcome, Outcome::Changed);
    assert!(source.exists());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
}

#[test]
fn cancellation_after_child_removal_is_a_proven_retained_directory_partial() {
    let (root, plan) = fixture(Operation::Move);
    let source = root.path().join("source");
    let cancel = AtomicBool::new(false);
    let record = execute(&plan, Options::default(), &cancel, |event| {
        if matches!(event, Event::Progress { stage: Stage::Removing, source: ref path, .. } if path == &source)
        {
            assert!(!source.join("child").exists());
            cancel.store(true, Ordering::Relaxed);
        }
        Decision::Skip
    });
    assert!(cancel.load(Ordering::Relaxed));
    assert_partial(&plan, &record);
    assert_eq!(
        fs::read(root.path().join("target/source/child")).unwrap(),
        b"completed child"
    );
    assert!(source.is_dir());
}

#[test]
fn source_replacement_after_completed_children_remains_unknown_and_quarantined() {
    let (root, plan) = fixture(Operation::Move);
    let source = root.path().join("source");
    let held = root.path().join("original-directory");
    let record = execute(&plan, Options::default(), &false.into(), |event| {
        if matches!(event, Event::Progress { stage: Stage::Removing, source: ref path, .. } if path == &source)
        {
            assert!(!source.join("child").exists());
            fs::rename(&source, &held).unwrap();
            fs::create_dir(&source).unwrap();
            fs::write(source.join("foreign"), "do not touch").unwrap();
        }
        Decision::Skip
    });
    assert_eq!(record.entries[0].outcome, Outcome::Completed);
    assert_eq!(record.entries[1].outcome, Outcome::OutcomeUnknown);
    assert!(record.entries[1].recovery.is_some());
    assert!(record.reconcilable());
    assert!(plan.retry(&record).is_err());
    let checked = terminal_file_manager::operations::reconcile(&plan, record, &false.into());
    assert!(checked.unresolved());
    let job = job(plan.clone(), checked);
    assert_eq!(job.state, JobState::Unknown);
    assert!(!job.can_retry());
    assert!(job.can_reconcile());
    assert_eq!(fs::read(source.join("foreign")).unwrap(), b"do not touch");
    assert!(held.is_dir());
    assert_eq!(
        terminal_file_manager::fs::inspect(&held)
            .identity
            .unwrap()
            .object,
        plan.operands[0].identity.as_ref().unwrap().object
    );
    assert_eq!(
        fs::read(root.path().join("target/source/child")).unwrap(),
        b"completed child"
    );
}

#[test]
fn cancelled_copy_retains_both_payloads_and_directory_retry_evidence() {
    let (root, plan) = fixture(Operation::Copy);
    let cancel = AtomicBool::new(false);
    let record = execute(&plan, Options::default(), &cancel, |event| {
        if matches!(
            event,
            Event::Progress {
                stage: Stage::Committed,
                ..
            }
        ) {
            cancel.store(true, Ordering::Relaxed);
        }
        Decision::Skip
    });
    assert_partial(&plan, &record);
    for path in ["source/child", "target/source/child"] {
        assert_eq!(
            fs::read(root.path().join(path)).unwrap(),
            b"completed child"
        );
    }
    let retry = plan.retry(&record).unwrap();
    let completed = execute(&retry, Options::default(), &false.into(), |_| {
        panic!("completed child repeated")
    });
    assert_eq!(completed.entries.len(), 1);
    assert_eq!(completed.entries[0].outcome, Outcome::Completed);
}
