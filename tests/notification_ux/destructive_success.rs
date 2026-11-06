use super::*;
use terminal_file_manager::operations::{
    EntryResult, JobState, Operation, Outcome, Plan, Record, Update,
};

fn finish(c: &mut Controller, plan: Plan, outcome: Outcome) {
    let entries = plan
        .operands
        .iter()
        .map(|entry| EntryResult {
            source: entry.path.clone(),
            destination: None,
            kind: entry.kind,
            outcome,
            bytes: 0,
            checksum_verified: false,
            created_directory: false,
            detail: "Retained native evidence".into(),
            trash: None,
            recovery: None,
        })
        .collect();
    c.register_operation(7, plan, Default::default(), None);
    c.complete_operation(Update::Finished {
        id: 7,
        record: Record { entries },
    });
}

#[test]
fn destructive_success_names_files_directories_and_multiple_selected_items() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("report.txt");
    let folder = root.path().join("資料");
    std::fs::write(&file, "fixture").unwrap();
    std::fs::create_dir(&folder).unwrap();
    for (operation, ending) in [
        (Operation::Trash, "sent to Trash"),
        (Operation::Delete, "permanently deleted"),
    ] {
        for (paths, name) in [
            (vec![file.clone()], "“report.txt”"),
            (vec![folder.clone()], "“資料”"),
            (vec![file.clone(), folder.clone()], "2 items"),
        ] {
            let mut c = controller();
            finish(
                &mut c,
                Plan::paths(operation, paths, None).unwrap(),
                Outcome::Completed,
            );
            let event = &c.app.notifications.history[0];
            assert_eq!(event.severity, Severity::Success);
            assert_eq!(event.presentation.title, "Success");
            assert_eq!(event.presentation.message, format!("{name} {ending}"));
            assert_eq!(event.job, Some(7));
            let job = &c.app.operations.history[0];
            assert_eq!(job.state, JobState::Completed);
            let record = job.record.as_ref().unwrap();
            assert_eq!(record.entries.len(), job.plan.operands.len());
            assert!(
                record
                    .entries
                    .iter()
                    .all(|entry| entry.detail == "Retained native evidence")
            );
            let (buffer, geometry) = draw(&c, 80);
            let card = text(&buffer, geometry.toasts[0].area);
            assert!(card.contains("Success"));
            assert!(!card.contains("Success ·"));
            assert!(card.contains(ending));
            assert!(card.contains("F9 details"));
        }
    }
}

#[test]
fn destructive_non_success_outcomes_never_claim_success() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("owned");
    std::fs::write(&file, "fixture").unwrap();
    for operation in [Operation::Trash, Operation::Delete] {
        for outcome in [
            Outcome::Failed,
            Outcome::Skipped,
            Outcome::Cancelled,
            Outcome::OutcomeUnknown,
            Outcome::SafetyIncident,
        ] {
            let mut c = controller();
            finish(
                &mut c,
                Plan::paths(operation, vec![file.clone()], None).unwrap(),
                outcome,
            );
            let event = &c.app.notifications.history[0];
            assert_ne!(event.severity, Severity::Success);
            assert_ne!(event.presentation.title, "Success");
            assert!(!event.presentation.message.contains("sent to Trash"));
            assert_eq!(
                c.app.operations.history[0].record.as_ref().unwrap().entries[0].outcome,
                outcome
            );
        }
    }
}
