use std::{
    fs,
    time::{Duration, SystemTime},
};
use terminal_file_manager::operations::{Job, JobState, Operation, Outcome, Plan, Record, execute};

fn fixture() -> (tempfile::TempDir, Job, Record) {
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
    let record = execute(&plan, Default::default(), &false.into(), |_| {
        terminal_file_manager::operations::Decision::Skip
    });
    let job = Job {
        presentation: None,
        retry_eligible: false,
        id: 1,
        plan,
        options: Default::default(),
        state: JobState::Running,
        record: None,
        progress: None,
        started: SystemTime::UNIX_EPOCH,
        finished: None,
        retry_of: None,
    };
    (root, job, record)
}

#[test]
fn an_all_skipped_conflict_job_never_claims_completion() {
    let (root, mut job, record) = fixture();
    assert_eq!(record.entries[0].outcome, Outcome::Skipped);
    assert_eq!(record.completed_files(), 0);
    job.finish(record, job.started + Duration::from_secs(2));
    assert_eq!(job.state, JobState::Skipped);
    assert_eq!(
        fs::read(root.path().join("target/source")).unwrap(),
        b"existing"
    );
}

#[test]
fn job_outcome_precedence_keeps_partial_unknown_and_critical_distinct() {
    use Outcome::*;
    let cases = [
        (vec![], JobState::Skipped),
        (vec![Completed], JobState::Completed),
        (vec![Completed, Skipped], JobState::Partial),
        (vec![Changed], JobState::Failed),
        (vec![Failed], JobState::Failed),
        (vec![CopiedSourceRetained], JobState::Partial),
        (vec![Completed, Failed], JobState::Partial),
        (vec![Cancelled, Failed], JobState::Cancelled),
        (vec![OutcomeUnknown, Cancelled], JobState::Unknown),
        (
            vec![SafetyIncident, OutcomeUnknown, Completed],
            JobState::Critical,
        ),
    ];
    let (_, mut job, record) = fixture();
    for (outcomes, expected) in cases {
        let entries = outcomes
            .into_iter()
            .map(|outcome| {
                let mut entry = record.entries[0].clone();
                entry.outcome = outcome;
                entry
            })
            .collect();
        job.finish(Record { entries }, job.started + Duration::from_secs(4));
        assert_eq!(job.state, expected);
        assert_eq!(
            job.can_retry(),
            job.plan.retry(job.record.as_ref().unwrap()).is_ok()
        );
    }
}

#[test]
fn stopped_job_actions_exclude_completed_unknown_and_critical_results() {
    use Outcome::*;
    let (_, mut job, record) = fixture();
    assert!(!job.can_retry() && !job.can_reconcile());
    for (outcome, retry) in [
        (Completed, false),
        (Skipped, true),
        (Failed, true),
        (Changed, true),
        (Cancelled, true),
        (CopiedSourceRetained, true),
        (OutcomeUnknown, false),
        (SafetyIncident, false),
    ] {
        let mut entry = record.entries[0].clone();
        entry.outcome = outcome;
        job.finish(
            Record {
                entries: vec![entry],
            },
            job.started,
        );
        assert!(!job.active());
        assert_eq!(job.can_retry(), retry, "{outcome:?}");
        assert_eq!(
            job.can_retry(),
            job.plan.retry(job.record.as_ref().unwrap()).is_ok()
        );
        assert!(!job.can_reconcile());
    }
}

#[test]
fn retry_eligibility_queries_do_not_repeat_large_preview_clones() {
    use terminal_file_manager::operations::{Queue, Update};
    let (root, mut job, _) = fixture();
    let selected = root.path().join("selected");
    fs::create_dir(&selected).unwrap();
    for index in 0..2000 {
        fs::write(selected.join(index.to_string()), "retained").unwrap();
    }
    let plan = Plan::paths(Operation::Delete, vec![selected], None).unwrap();
    let mut queue = Queue::new().unwrap();
    queue.preview(plan).unwrap();
    job.plan = loop {
        if let Update::Preview { plan, .. } = queue.next(Duration::from_secs(10)).unwrap() {
            break *plan;
        }
    };
    let record = execute(&job.plan, Default::default(), &true.into(), |_| {
        terminal_file_manager::operations::Decision::Skip
    });
    job.finish(record, job.started);
    let baseline = std::time::Instant::now();
    for _ in 0..100 {
        assert!(std::hint::black_box(
            job.plan.retry(job.record.as_ref().unwrap()).is_ok()
        ));
    }
    let baseline = baseline.elapsed();
    let cached = std::time::Instant::now();
    for _ in 0..10_000 {
        assert!(std::hint::black_box(job.can_retry()));
    }
    eprintln!(
        "RETRY_COST: 2000-entry preview, 100 original queries {baseline:?}, 10000 cached queries {:?}",
        cached.elapsed()
    );
    assert_eq!(job.plan.preview.as_ref().unwrap().files, 2000);
    assert_eq!(
        fs::read(job.plan.operands[0].path.join("0")).unwrap(),
        b"retained"
    );
}
