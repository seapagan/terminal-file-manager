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
        assert!(!job.can_reconcile());
    }
}
