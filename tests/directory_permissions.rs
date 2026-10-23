#![cfg(unix)]

use std::{
    fs,
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use terminal_file_manager::operations::{
    Decision, Event, Operation, Options, Outcome, Plan, Stage, execute,
};

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("source/nested")).unwrap();
    fs::create_dir(root.path().join("target")).unwrap();
    for name in ["source", "source/nested"] {
        fs::set_permissions(root.path().join(name), fs::Permissions::from_mode(0o700)).unwrap();
    }
    let secret = root.path().join("source/nested/secret");
    fs::write(&secret, b"private bytes").unwrap();
    fs::set_permissions(secret, fs::Permissions::from_mode(0o644)).unwrap();
    root
}

fn plan(root: &Path, operation: Operation) -> Plan {
    Plan::paths(
        operation,
        vec![root.join("source")],
        Some(root.join("target")),
    )
    .unwrap()
}

fn cancelled_copy(root: &Path, destination: &Path, operation: Operation) {
    let cancel = AtomicBool::new(false);
    let mut observed = None;
    let plan = Plan::paths(
        operation,
        vec![root.join("source")],
        Some(destination.into()),
    )
    .unwrap();
    let record = execute(&plan, Options { checksum: true }, &cancel, |event| {
        if let Event::Progress {
            stage: Stage::Committed,
            destination,
            ..
        } = event
        {
            let nested = destination.parent().unwrap();
            observed = Some((
                mode(nested),
                mode(nested.parent().unwrap()),
                fs::read(destination).unwrap(),
            ));
            cancel.store(true, Ordering::Relaxed);
        }
        Decision::Skip
    });
    assert_eq!(observed, Some((0o700, 0o700, b"private bytes".to_vec())));
    for name in ["source", "source/nested"] {
        assert_eq!(mode(&destination.join(name)), 0o700, "{record:?}");
    }
    assert_eq!(
        fs::read(destination.join("source/nested/secret")).unwrap(),
        b"private bytes"
    );
    assert_eq!(
        fs::read(root.join("source/nested/secret")).unwrap(),
        b"private bytes"
    );
    assert!(record.entries.iter().any(|entry| matches!(
        entry.outcome,
        Outcome::Cancelled | Outcome::CopiedSourceRetained
    )));
}

#[test]
fn cancelled_recursive_copy_and_move_keep_new_ancestors_private() {
    for operation in [Operation::Copy, Operation::Move] {
        let root = fixture();
        cancelled_copy(root.path(), &root.path().join("target"), operation);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn cancelled_cross_filesystem_directory_move_keeps_new_ancestors_private() {
    use std::os::unix::fs::MetadataExt;
    let root = fixture();
    let target = tempfile::tempdir_in("/dev/shm").unwrap();
    assert_ne!(
        fs::metadata(root.path()).unwrap().dev(),
        fs::metadata(target.path()).unwrap().dev()
    );
    cancelled_copy(root.path(), target.path(), Operation::Move);
}

#[test]
fn failed_child_keeps_new_directories_private_and_copied_bytes_intact() {
    let root = fixture();
    let _socket = UnixListener::bind(root.path().join("source/nested/socket")).unwrap();
    let record = execute(
        &plan(root.path(), Operation::Copy),
        Options::default(),
        &false.into(),
        |_| Decision::Skip,
    );
    assert!(
        record
            .entries
            .iter()
            .any(|entry| entry.outcome == Outcome::Failed)
    );
    assert_eq!(mode(&root.path().join("target/source")), 0o700);
    assert_eq!(mode(&root.path().join("target/source/nested")), 0o700);
    assert_eq!(
        fs::read(root.path().join("target/source/nested/secret")).unwrap(),
        b"private bytes"
    );
}

#[test]
fn completed_directories_copy_source_modes_but_merges_preserve_existing_modes() {
    for operation in [Operation::Copy, Operation::Move] {
        for merge in [false, true] {
            let root = fixture();
            fs::set_permissions(
                root.path().join("source"),
                fs::Permissions::from_mode(0o750),
            )
            .unwrap();
            if merge {
                fs::create_dir_all(root.path().join("target/source/nested")).unwrap();
                for name in ["target/source", "target/source/nested"] {
                    fs::set_permissions(root.path().join(name), fs::Permissions::from_mode(0o711))
                        .unwrap();
                }
            }
            let record = execute(
                &plan(root.path(), operation),
                Options { checksum: true },
                &false.into(),
                |_| Decision::Skip,
            );
            assert!(
                record
                    .entries
                    .iter()
                    .all(|entry| entry.outcome == Outcome::Completed),
                "{record:?}"
            );
            assert_eq!(
                mode(&root.path().join("target/source")),
                if merge { 0o711 } else { 0o750 }
            );
            assert_eq!(
                mode(&root.path().join("target/source/nested")),
                if merge { 0o711 } else { 0o700 }
            );
            assert_eq!(
                fs::read(root.path().join("target/source/nested/secret")).unwrap(),
                b"private bytes"
            );
            assert_eq!(
                root.path().join("source").exists(),
                operation == Operation::Copy
            );
        }
    }
}
