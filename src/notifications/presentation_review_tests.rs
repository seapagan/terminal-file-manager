use super::*;
use crate::operations::{Options, Plan};
use std::time::SystemTime;

fn completed(operation: Operation, count: usize) -> (tempfile::TempDir, Job) {
    let root = tempfile::tempdir().unwrap();
    let mut plan = Plan::mkdir(root.path().join("destination"));
    plan.operation = operation;
    for index in 0..count {
        let path = root.path().join(format!("source-{index}"));
        std::fs::write(&path, "data").unwrap();
        plan.operands.push(crate::fs::inspect(&path));
    }
    let job = Job {
        presentation: None,
        retry_eligible: false,
        id: 1,
        plan,
        options: Options::default(),
        state: JobState::Completed,
        record: None,
        progress: None,
        started: SystemTime::UNIX_EPOCH,
        finished: Some(SystemTime::UNIX_EPOCH),
        retry_of: None,
    };
    (root, job)
}

#[test]
fn rename_success_identifies_resulting_name() {
    let (_root, job) = completed(Operation::Rename, 1);
    assert_eq!(Presentation::job(&job).message, "“destination”");
}

#[test]
fn transfer_success_identifies_destination_and_scope() {
    for operation in [Operation::Copy, Operation::Move] {
        let (_root, mut job) = completed(operation, 1);
        assert_eq!(
            Presentation::job(&job).message,
            "“source-0” to “destination”"
        );
        job.plan.incoming_name = Some("renamed".into());
        assert_eq!(
            Presentation::job(&job).message,
            "“renamed” to “destination”"
        );
        let (_root, job) = completed(operation, 2);
        assert_eq!(
            Presentation::job(&job).message,
            "2 selected entries to “destination”"
        );
    }
}

#[test]
fn permissions_and_destructive_success_keep_source_scope() {
    for operation in [Operation::Permissions(0o600), Operation::ReadOnly(true)] {
        let (_root, job) = completed(operation, 1);
        assert_eq!(Presentation::job(&job).message, "“source-0”");
    }
    for (operation, ending) in [
        (Operation::Trash, "sent to Trash"),
        (Operation::Delete, "permanently deleted"),
    ] {
        let (_root, job) = completed(operation, 1);
        assert_eq!(
            Presentation::job(&job).message,
            format!("“source-0” {ending}")
        );
        let (_root, job) = completed(operation, 2);
        assert_eq!(Presentation::job(&job).message, format!("2 items {ending}"));
    }
}

#[test]
fn trash_summary_counts_recycled_permanent_and_mixed_outcomes() {
    use crate::operations::Decision;
    let (_root, mut job) = completed(Operation::Trash, 2);
    let record = crate::operations::execute(&job.plan, Options::default(), &true.into(), |_| {
        Decision::Skip
    });
    for (outcomes, expected) in [
        (
            vec![Outcome::Completed],
            "Job 1: 1 entry trashed (native top-level acknowledgements)",
        ),
        (
            vec![Outcome::PermanentlyDeleted],
            "Job 1: 1 permanently deleted (irreversible)",
        ),
        (
            vec![Outcome::Completed, Outcome::Completed],
            "Job 1: 2 entries trashed (native top-level acknowledgements)",
        ),
        (
            vec![Outcome::PermanentlyDeleted, Outcome::PermanentlyDeleted],
            "Job 1: 2 permanently deleted (irreversible)",
        ),
        (
            vec![Outcome::Completed, Outcome::PermanentlyDeleted],
            "Job 1: 1 entry trashed (native top-level acknowledgements); 1 permanently deleted (irreversible)",
        ),
    ] {
        let mut record = record.clone();
        record.entries.truncate(outcomes.len());
        for (entry, outcome) in record.entries.iter_mut().zip(outcomes) {
            entry.outcome = outcome;
        }
        job.finish(record, SystemTime::UNIX_EPOCH);
        assert_eq!(crate::operations::summary(&job), expected);
    }
}

#[test]
fn permanent_trash_success_is_distinct_and_never_retried() {
    use crate::operations::Decision;
    let (_root, mut job) = completed(Operation::Trash, 2);
    let mut record =
        crate::operations::execute(&job.plan, Options::default(), &true.into(), |_| {
            Decision::Skip
        });
    record.entries[0].outcome = Outcome::PermanentlyDeleted;
    record.entries[1].outcome = Outcome::Completed;
    job.finish(record.clone(), SystemTime::UNIX_EPOCH);
    assert_eq!(job.state, JobState::Completed);
    assert!(!job.can_retry());
    assert!(!job.can_reconcile());
    assert!(!record.unresolved());
    assert!(!record.incident());
    assert!(record.failure_groups().is_empty());
    assert_eq!(job.presentation().all_completed, None);
    let notice = Presentation::job(&job);
    assert_eq!(notice.title, "Success");
    assert!(
        notice.message.contains("1 entry trashed"),
        "{}",
        notice.message
    );
    assert!(
        notice
            .message
            .contains("1 permanently deleted (irreversible)")
    );
    record.entries[1].outcome = Outcome::Failed;
    job.finish(record.clone(), SystemTime::UNIX_EPOCH);
    assert_eq!(job.state, JobState::Partial);
    let retry = job.plan.retry(&record).unwrap();
    assert_eq!(retry.operands.len(), 1);
    assert_eq!(retry.operands[0].path, record.entries[1].source);
    record.entries[1].outcome = Outcome::PermanentlyDeleted;
    job.finish(record, SystemTime::UNIX_EPOCH);
    assert!(!Presentation::job(&job).message.contains("sent to Trash"));
}

#[test]
fn diagnostic_rewriting_preserves_error_meaning() {
    for (detail, expected) in [
        ("Is a directory (os error 21)", "Is a directory"),
        (
            "Source and destination have incompatible types.",
            "Source and destination have incompatible types.",
        ),
        (
            "File exists but cannot be inspected",
            "File exists but cannot be inspected",
        ),
        (
            "Permission denied while reading metadata",
            "Permission denied while reading metadata",
        ),
        (
            "Cannot copy /one/source to /two/target",
            "Cannot copy source to target",
        ),
        (
            r"Cannot copy C:\one\source to D:\two\target",
            "Cannot copy source to target",
        ),
        (
            "Failed (os error 21) while inspecting",
            "Failed (os error 21) while inspecting",
        ),
    ] {
        assert_eq!(readable(detail), expected);
    }
}

#[test]
fn known_self_target_and_access_errors_remain_concise() {
    for detail in [
        "Source and destination are the same.",
        "Source and destination are the same folder.",
        "Source and destination refer to the same item.",
    ] {
        assert_eq!(readable(detail), "Source and destination are the same.");
    }
    assert_eq!(
        readable("Access is denied. (os error 5)"),
        "Permission denied."
    );
}
