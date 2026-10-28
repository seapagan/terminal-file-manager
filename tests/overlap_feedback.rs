use std::path::Path;
use terminal_file_manager::operations::{Decision, Operation, Outcome, Plan, Record, execute};

fn run(
    operation: Operation,
    source: &Path,
    destination: &Path,
    incoming: Option<&str>,
    choice: Decision,
) -> Record {
    let mut plan = Plan::paths(operation, vec![source.into()], Some(destination.into())).unwrap();
    plan.incoming_name = incoming.map(Into::into);
    execute(&plan, Default::default(), &false.into(), |_| choice.clone())
}

#[test]
fn directory_self_and_descendant_refusals_keep_the_tree_intact() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    std::fs::create_dir_all(folder.join("nested")).unwrap();
    std::fs::write(folder.join("retained"), "owned").unwrap();
    for operation in [Operation::Copy, Operation::Move] {
        for (target, reason) in [
            (root.path(), "Source and destination are the same folder."),
            (folder.as_path(), "Destination is inside the source folder."),
            (
                folder.join("nested").as_path(),
                "Destination is inside the source folder.",
            ),
        ] {
            let record = run(operation, &folder, target, None, Decision::Replace);
            assert_eq!(record.entries[0].outcome, Outcome::Failed);
            assert_eq!(record.entries[0].detail, reason);
            assert_eq!(std::fs::read(folder.join("retained")).unwrap(), b"owned");
            assert!(!folder.join("folder").exists());
        }
    }
}

#[test]
fn hard_link_aliases_are_explained_without_replacing_either_name() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let alias = root.path().join("alias");
    std::fs::write(&source, b"owned").unwrap();
    std::fs::hard_link(&source, &alias).unwrap();
    for operation in [Operation::Copy, Operation::Move] {
        let record = run(
            operation,
            &source,
            root.path(),
            Some("alias"),
            Decision::Replace,
        );
        assert_eq!(record.entries[0].outcome, Outcome::Failed);
        assert_eq!(
            record.entries[0].detail,
            "Source and destination refer to the same item."
        );
        assert_eq!(std::fs::read(&source).unwrap(), b"owned");
        assert_eq!(std::fs::read(&alias).unwrap(), b"owned");
    }
}

#[cfg(unix)]
#[test]
fn symlink_parent_alias_refusal_preserves_native_spelling_and_data() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("parent");
    std::fs::create_dir(&parent).unwrap();
    let source = parent.join("source");
    std::fs::write(&source, b"owned").unwrap();
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(&parent, &alias).unwrap();
    for operation in [Operation::Copy, Operation::Move] {
        let record = run(operation, &source, &alias, None, Decision::Replace);
        assert_eq!(
            record.entries[0].detail,
            "Source and destination refer to the same item."
        );
        assert_eq!(
            record.entries[0].destination.as_ref().unwrap(),
            &alias.join("source")
        );
        assert_eq!(std::fs::read(&source).unwrap(), b"owned");
    }
}

#[test]
fn same_parent_allows_explicit_names_and_keep_both_outputs() {
    for operation in [Operation::Copy, Operation::Move] {
        for choice in [Decision::Replace, Decision::KeepBoth] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            std::fs::write(&source, b"incoming").unwrap();
            if matches!(choice, Decision::KeepBoth) {
                std::fs::write(root.path().join("renamed"), b"existing").unwrap();
            }
            let record = run(
                operation,
                &source,
                root.path(),
                Some("renamed"),
                choice.clone(),
            );
            assert_eq!(record.entries[0].outcome, Outcome::Completed, "{record:?}");
            let target = record.entries[0].destination.as_ref().unwrap();
            assert_ne!(target, &source);
            assert_eq!(std::fs::read(target).unwrap(), b"incoming");
            assert_eq!(source.exists(), operation == Operation::Copy);
            if matches!(choice, Decision::KeepBoth) {
                assert_eq!(
                    std::fs::read(root.path().join("renamed")).unwrap(),
                    b"existing"
                );
            }
        }
    }
}

#[test]
fn case_only_destinations_follow_native_identity_without_case_guessing() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("SOURCE");
    std::fs::write(&source, b"incoming").unwrap();
    let a = terminal_file_manager::fs::inspect(&source);
    let b = terminal_file_manager::fs::inspect(&target);
    let alias = b
        .identity
        .as_ref()
        .is_some_and(|id| id.object == a.identity.as_ref().unwrap().object);
    let record = run(
        Operation::Copy,
        &source,
        root.path(),
        Some("SOURCE"),
        Decision::Replace,
    );
    assert_eq!(
        record.entries[0].outcome,
        if alias {
            Outcome::Failed
        } else {
            Outcome::Completed
        }
    );
    if alias {
        assert_eq!(
            record.entries[0].detail,
            "Source and destination refer to the same item."
        );
    }
    assert_eq!(std::fs::read(source).unwrap(), b"incoming");
    assert_eq!(std::fs::read(target).unwrap(), b"incoming");
}
