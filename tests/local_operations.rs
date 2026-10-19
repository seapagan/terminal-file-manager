use std::{fs, sync::atomic::AtomicBool};
use terminal_file_manager::operations::{
    Decision, Event, Operation, Options, Outcome, Plan, Stage, execute,
};

fn transfer(root: &std::path::Path, operation: Operation) -> Plan {
    Plan::paths(
        operation,
        vec![root.join("source")],
        Some(root.join("destination")),
    )
    .unwrap()
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), b"new bytes").unwrap();
    fs::create_dir(root.path().join("destination")).unwrap();
    root
}

#[test]
fn copy_verifies_staged_bytes_and_never_clobbers_without_a_conflict_decision() {
    let root = fixture();
    let destination = root.path().join("destination/source");
    fs::write(&destination, "old bytes").unwrap();
    let record = execute(
        &transfer(root.path(), Operation::Copy),
        Options::default(),
        &AtomicBool::new(false),
        |_| Decision::Skip,
    );
    assert_eq!(fs::read(&destination).unwrap(), b"old bytes");
    assert_eq!(record.entries[0].outcome, Outcome::Skipped);
    let record = execute(
        &transfer(root.path(), Operation::Copy),
        Options { checksum: true },
        &AtomicBool::new(false),
        |event| {
            if matches!(
                event,
                Event::Progress {
                    stage: Stage::Verified,
                    ..
                }
            ) {
                assert_eq!(fs::read(&destination).unwrap(), b"old bytes");
            }
            Decision::Replace
        },
    );
    assert_eq!(fs::read(destination).unwrap(), b"new bytes");
    assert_eq!(record.entries[0].outcome, Outcome::Completed);
    assert!(record.entries[0].checksum_verified);
    assert_eq!(record.bytes(), 9);
}

#[test]
fn recursive_move_merges_and_retains_skipped_children_and_their_parent() {
    let root = fixture();
    fs::remove_file(root.path().join("source")).unwrap();
    fs::create_dir_all(root.path().join("source/nested")).unwrap();
    fs::create_dir_all(root.path().join("destination/source/nested")).unwrap();
    fs::write(root.path().join("source/nested/keep"), "incoming").unwrap();
    fs::write(root.path().join("source/nested/move"), "move").unwrap();
    fs::write(
        root.path().join("destination/source/nested/keep"),
        "existing",
    )
    .unwrap();
    let record = execute(
        &transfer(root.path(), Operation::Move),
        Options::default(),
        &false.into(),
        |_| Decision::Skip,
    );
    assert!(root.path().join("source/nested/keep").exists());
    assert!(!root.path().join("source/nested/move").exists());
    assert_eq!(
        fs::read(root.path().join("destination/source/nested/keep")).unwrap(),
        b"existing"
    );
    assert_eq!(
        fs::read(root.path().join("destination/source/nested/move")).unwrap(),
        b"move"
    );
    assert_eq!(record.completed_files(), 1);
    assert_eq!(record.completed_directories(), 0);
}

#[test]
fn changed_source_or_destination_blocks_finalization_and_preserves_data() {
    for change_source in [true, false] {
        let root = fixture();
        let destination = root.path().join("destination/source");
        fs::write(&destination, "previous").unwrap();
        let record = execute(
            &transfer(root.path(), Operation::Move),
            Options::default(),
            &false.into(),
            |event| {
                if matches!(
                    event,
                    Event::Progress {
                        stage: Stage::Verified,
                        ..
                    }
                ) {
                    fs::write(
                        if change_source {
                            root.path().join("source")
                        } else {
                            destination.clone()
                        },
                        "external change",
                    )
                    .unwrap();
                }
                Decision::Replace
            },
        );
        assert!(root.path().join("source").exists());
        assert_eq!(
            fs::read(&destination).unwrap(),
            if change_source {
                b"previous".as_slice()
            } else {
                b"external change".as_slice()
            }
        );
        assert_eq!(record.entries[0].outcome, Outcome::Changed);
        assert_eq!(
            fs::read_dir(root.path().join("destination"))
                .unwrap()
                .count(),
            1
        );
    }
}

#[test]
fn cancellation_acknowledges_owned_stage_cleanup_without_source_removal() {
    let root = fixture();
    let cancel = AtomicBool::new(false);
    let record = execute(
        &transfer(root.path(), Operation::Move),
        Options::default(),
        &cancel,
        |event| {
            if matches!(
                event,
                Event::Progress {
                    stage: Stage::Verified,
                    ..
                }
            ) {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Decision::Skip
        },
    );
    assert!(root.path().join("source").exists());
    assert_eq!(record.entries[0].outcome, Outcome::Cancelled);
    assert_eq!(
        fs::read_dir(root.path().join("destination"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn committed_destination_change_retains_move_source_and_reports_partial_move() {
    let root = fixture();
    let record = execute(
        &transfer(root.path(), Operation::Move),
        Options::default(),
        &false.into(),
        |event| {
            if matches!(
                event,
                Event::Progress {
                    stage: Stage::Committed,
                    ..
                }
            ) {
                fs::write(root.path().join("destination/source"), "external overwrite").unwrap();
            }
            Decision::Skip
        },
    );
    assert!(root.path().join("source").exists());
    assert_eq!(record.entries[0].outcome, Outcome::CopiedSourceRetained);
    assert_eq!(record.completed_files(), 0);
}

#[test]
fn destination_replacement_at_source_removal_boundary_retains_source() {
    let root = fixture();
    let record = execute(
        &transfer(root.path(), Operation::Move),
        Options::default(),
        &false.into(),
        |event| {
            if matches!(
                event,
                Event::Progress {
                    stage: Stage::Removing,
                    ..
                }
            ) {
                fs::write(root.path().join("destination/source"), "external overwrite").unwrap();
            }
            Decision::Skip
        },
    );
    assert!(root.path().join("source").exists());
    assert_eq!(record.entries[0].outcome, Outcome::CopiedSourceRetained);
}

#[cfg(unix)]
#[test]
fn replaced_ancestor_never_redirects_commit_or_source_removal() {
    let root = fixture();
    fs::create_dir(root.path().join("input")).unwrap();
    fs::rename(root.path().join("source"), root.path().join("input/source")).unwrap();
    let plan = Plan::paths(
        Operation::Move,
        vec![root.path().join("input/source")],
        Some(root.path().join("destination")),
    )
    .unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |event| {
        if matches!(
            event,
            Event::Progress {
                stage: Stage::Verified,
                ..
            }
        ) {
            fs::rename(
                root.path().join("input"),
                root.path().join("retained-input"),
            )
            .unwrap();
            std::os::unix::fs::symlink("retained-input", root.path().join("input")).unwrap();
        }
        Decision::Skip
    });
    assert!(root.path().join("retained-input/source").exists());
    assert!(!root.path().join("destination/source").exists());
    assert_eq!(record.entries[0].outcome, Outcome::Changed);
}

#[test]
fn aliases_and_self_descendants_are_rejected_before_any_mutation() {
    let root = fixture();
    fs::hard_link(
        root.path().join("source"),
        root.path().join("destination/source"),
    )
    .unwrap();
    let record = execute(
        &transfer(root.path(), Operation::Copy),
        Options::default(),
        &false.into(),
        |_| Decision::Replace,
    );
    assert_eq!(record.entries[0].outcome, Outcome::Failed);
    assert_eq!(fs::read(root.path().join("source")).unwrap(), b"new bytes");
    fs::remove_file(root.path().join("source")).unwrap();
    fs::create_dir_all(root.path().join("source/child")).unwrap();
    let plan = Plan::paths(
        Operation::Copy,
        vec![root.path().join("source")],
        Some(root.path().join("source/child")),
    )
    .unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| {
        Decision::Replace
    });
    assert_eq!(record.entries[0].outcome, Outcome::Failed);
    assert!(!root.path().join("source/child/source").exists());
}

#[cfg(unix)]
#[test]
fn links_are_copied_as_links_and_special_files_never_open_as_streams() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    fs::remove_file(root.path().join("source")).unwrap();
    symlink("missing-target", root.path().join("source")).unwrap();
    let record = execute(
        &transfer(root.path(), Operation::Copy),
        Options::default(),
        &false.into(),
        |_| Decision::Skip,
    );
    assert_eq!(record.entries[0].outcome, Outcome::Completed);
    assert_eq!(
        fs::read_link(root.path().join("destination/source")).unwrap(),
        std::path::PathBuf::from("missing-target")
    );
    fs::remove_file(root.path().join("source")).unwrap();
    let _socket = std::os::unix::net::UnixListener::bind(root.path().join("source")).unwrap();
    let record = execute(
        &transfer(root.path(), Operation::Copy),
        Options::default(),
        &false.into(),
        |_| Decision::Replace,
    );
    assert_eq!(record.entries[0].outcome, Outcome::Failed);
    assert!(fs::symlink_metadata(root.path().join("source")).is_ok());
}
