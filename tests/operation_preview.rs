use std::{fs, time::Duration};
use terminal_file_manager::operations::{
    Decision, Operation, Options, Outcome, Plan, Queue, Update, execute,
};

#[test]
fn delete_preview_counts_without_mutation_and_detects_later_descendant_changes() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("selected/nested")).unwrap();
    fs::write(root.path().join("selected/nested/original"), "preserved").unwrap();
    let plan = Plan::paths(Operation::Delete, vec![root.path().join("selected")], None).unwrap();
    let mut queue = Queue::new().unwrap();
    queue.preview(plan).unwrap();
    let plan = loop {
        if let Update::Preview { plan, .. } = queue.next(Duration::from_secs(5)).unwrap() {
            let counts = plan.preview.as_ref().unwrap();
            assert_eq!((counts.files, counts.directories), (1, 2));
            break plan;
        }
    };
    assert!(root.path().join("selected/nested/original").exists());
    fs::write(root.path().join("selected/nested/new"), "external").unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert_eq!(record.entries[0].outcome, Outcome::Changed);
    assert!(root.path().join("selected/nested/original").exists());
    assert!(root.path().join("selected/nested/new").exists());
}

#[test]
fn retry_skips_successful_entries_and_restarts_incomplete_work_from_scratch() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("source")).unwrap();
    fs::create_dir_all(root.path().join("target/source")).unwrap();
    for name in ["success", "skip"] {
        fs::write(root.path().join("source").join(name), name).unwrap();
    }
    fs::write(root.path().join("target/source/skip"), "existing").unwrap();
    let plan = Plan::paths(
        Operation::Copy,
        vec![root.path().join("source")],
        Some(root.path().join("target")),
    )
    .unwrap();
    let first = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert_eq!(first.completed_files(), 1);
    let retried = plan.retry(&first).unwrap();
    let second = execute(&retried, Options::default(), &false.into(), |_| {
        Decision::Replace
    });
    assert_eq!(second.completed_files(), 1);
    assert_eq!(
        fs::read(root.path().join("target/source/skip")).unwrap(),
        b"skip"
    );
    assert!(
        second
            .entries
            .iter()
            .all(|entry| entry.source.file_name().unwrap() != "success")
    );
}

#[test]
fn destructive_traversal_never_adopts_a_replacement_after_the_counted_preview() {
    let root = tempfile::tempdir().unwrap();
    let selected = root.path().join("selected");
    fs::create_dir(&selected).unwrap();
    for name in ["a", "b"] {
        fs::write(selected.join(name), "original").unwrap();
    }
    let plan = Plan::paths(Operation::Delete, vec![selected.clone()], None).unwrap();
    let mut queue = Queue::new().unwrap();
    queue.preview(plan).unwrap();
    let plan = loop {
        if let Update::Preview { plan, .. } = queue.next(Duration::from_secs(5)).unwrap() {
            break plan;
        }
    };
    let mut replaced = None;
    let record = execute(&plan, Options::default(), &false.into(), |event| {
        if let terminal_file_manager::operations::Event::Progress { source, .. } = event
            && replaced.is_none()
        {
            let other = selected.join(if source.file_name().unwrap() == "a" {
                "b"
            } else {
                "a"
            });
            fs::rename(&other, root.path().join("original-retained")).unwrap();
            fs::write(&other, "external replacement").unwrap();
            replaced = Some(other);
        }
        Decision::Skip
    });
    assert_eq!(
        fs::read(replaced.unwrap()).unwrap(),
        b"external replacement"
    );
    assert!(
        record
            .entries
            .iter()
            .any(|entry| entry.outcome == Outcome::Changed)
    );
}

#[test]
fn retry_a_failed_directory_creation_keeps_its_explicit_destination() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("parent/new");
    let plan = Plan::mkdir(destination.clone());
    let first = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert_eq!(first.entries[0].outcome, Outcome::Failed);
    fs::create_dir(root.path().join("parent")).unwrap();
    let retry = plan.retry(&first).unwrap();
    let second = execute(&retry, Options::default(), &false.into(), |_| {
        Decision::Skip
    });
    assert_eq!(second.completed_directories(), 1);
    assert!(destination.is_dir());
}
