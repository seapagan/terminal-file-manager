use std::fs;
use terminal_file_manager::operations::{
    Decision, Event, Operation, Options, Outcome, Plan, Stage, execute, reconcile,
};

#[test]
fn reconciliation_preserves_an_unowned_stage_then_cleans_only_the_restored_owned_object() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    fs::create_dir(&target).unwrap();
    let source = root.path().join("source");
    fs::write(&source, "original bytes").unwrap();
    let plan = Plan::paths(Operation::Copy, vec![source.clone()], Some(target.clone())).unwrap();
    let held = root.path().join("held-owned-stage");
    let mut stage_path = None;
    let record = execute(&plan, Options::default(), &false.into(), |event| {
        if matches!(
            event,
            Event::Progress {
                stage: Stage::Verified,
                ..
            }
        ) {
            let directory = stage_directory(&target);
            let payload = directory.join("payload");
            fs::rename(&payload, &held).unwrap();
            fs::write(&payload, "external replacement").unwrap();
            stage_path = Some(payload);
        }
        Decision::Skip
    });
    assert!(record.unresolved());
    let original_detail = record.entries[0].detail.clone();
    let stage_path = stage_path.unwrap();
    let unresolved = reconcile(&plan, record, &false.into());
    assert!(unresolved.unresolved());
    assert!(unresolved.entries[0].detail.contains(&original_detail));
    assert_eq!(fs::read(&stage_path).unwrap(), b"external replacement");
    fs::rename(&stage_path, root.path().join("external-preserved")).unwrap();
    fs::rename(&held, &stage_path).unwrap();
    let resolved = reconcile(&plan, unresolved, &false.into());
    assert!(!resolved.unresolved());
    assert_eq!(resolved.entries[0].outcome, Outcome::Failed);
    assert!(!stage_path.parent().unwrap().exists());
    assert_eq!(fs::read(source).unwrap(), b"original bytes");
    let retry = plan.retry(&resolved).unwrap();
    assert_eq!(
        execute(&retry, Options::default(), &false.into(), |_| {
            Decision::Skip
        })
        .completed_files(),
        1
    );
}

#[test]
fn reconciliation_of_a_committed_move_never_reissues_source_removal() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    fs::create_dir(&target).unwrap();
    let source = root.path().join("source");
    fs::write(&source, "verified bytes").unwrap();
    let plan = Plan::paths(Operation::Move, vec![source.clone()], Some(target.clone())).unwrap();
    let mut foreign = None;
    let record = execute(&plan, Options::default(), &false.into(), |event| {
        if matches!(
            event,
            Event::Progress {
                stage: Stage::Committed,
                ..
            }
        ) {
            let directory = stage_directory(&target);
            let path = directory.join("external");
            fs::write(&path, "preserve unrelated data").unwrap();
            foreign = Some(path);
        }
        Decision::Skip
    });
    assert!(record.unresolved());
    let record = reconcile(&plan, record, &false.into());
    assert!(record.unresolved());
    let foreign = foreign.unwrap();
    fs::rename(foreign, root.path().join("external-preserved")).unwrap();
    let record = reconcile(&plan, record, &false.into());
    assert_eq!(record.entries[0].outcome, Outcome::CopiedSourceRetained);
    assert_eq!(record.completed_files(), 0);
    assert_eq!(fs::read(source).unwrap(), b"verified bytes");
    assert_eq!(fs::read(target.join("source")).unwrap(), b"verified bytes");
}

fn stage_directory(parent: &std::path::Path) -> std::path::PathBuf {
    fs::read_dir(parent)
        .unwrap()
        .map(Result::unwrap)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".tfm-stage-")
        })
        .unwrap()
        .path()
}

#[test]
fn recursive_reconciliation_retains_parents_until_their_unknown_children_are_resolved() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::create_dir(&target).unwrap();
    fs::write(source.join("nested/file"), "tree bytes").unwrap();
    let plan = Plan::paths(Operation::Copy, vec![source.clone()], Some(target.clone())).unwrap();
    let mut foreign = None;
    let record = execute(&plan, Options::default(), &false.into(), |event| {
        if let Event::Progress {
            stage: Stage::Committed,
            destination,
            ..
        } = event
        {
            let path = stage_directory(destination.parent().unwrap()).join("external");
            fs::write(&path, "preserve").unwrap();
            foreign = Some(path);
        }
        Decision::Skip
    });
    assert!(record.unresolved());
    fs::rename(foreign.unwrap(), root.path().join("external-preserved")).unwrap();
    let record = reconcile(&plan, record, &false.into());
    assert!(!record.unresolved(), "{record:?}");
    assert_eq!(record.completed_files(), 1);
    assert_eq!(record.completed_directories(), 0);
    assert!(
        record
            .entries
            .iter()
            .filter(|entry| entry.kind == terminal_file_manager::fs::EntryKind::Directory)
            .all(|entry| entry.outcome == Outcome::CopiedSourceRetained)
    );
    assert_eq!(fs::read(source.join("nested/file")).unwrap(), b"tree bytes");
    assert_eq!(
        fs::read(target.join("source/nested/file")).unwrap(),
        b"tree bytes"
    );
}
