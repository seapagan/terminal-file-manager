use std::{fs, sync::atomic::AtomicBool};
#[cfg(target_os = "linux")]
mod common;
use terminal_file_manager::operations::{
    Decision, Event, Operation, Options, Outcome, Plan, Stage, execute,
};

#[cfg(windows)]
#[test]
fn native_readonly_survives_new_destination_commit_and_can_be_edited() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(&source, "readonly bytes").unwrap();
    let mut permissions = fs::metadata(&source).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&source, permissions).unwrap();
    let plan = Plan::paths(Operation::Copy, vec![source.clone()], Some(target.clone())).unwrap();
    let record = execute(&plan, Options { checksum: true }, &false.into(), |_| {
        Decision::Skip
    });
    assert_eq!(record.completed_files(), 1, "{record:?}");
    let copied = target.join("source");
    assert!(fs::metadata(&copied).unwrap().permissions().readonly());
    for path in [source, copied] {
        let plan = Plan::paths(Operation::ReadOnly(false), vec![path.clone()], None).unwrap();
        let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
        assert_eq!(record.completed_files(), 1, "{record:?}");
        assert!(!fs::metadata(path).unwrap().permissions().readonly());
    }
}

#[cfg(unix)]
#[test]
fn native_mode_mtime_and_supported_names_survive_verified_copy() {
    use std::{
        ffi::OsString,
        os::unix::{ffi::OsStringExt, fs::PermissionsExt},
    };
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let name = if cfg!(target_os = "linux") {
        OsString::from_vec(b"raw-\xff\nfile".to_vec())
    } else {
        OsString::from("native-λ\nfile")
    };
    let source = root.path().join(name);
    fs::write(&source, "bytes").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    let before = fs::metadata(&source).unwrap();
    let plan = Plan::paths(
        Operation::Copy,
        vec![source.clone()],
        Some(destination.clone()),
    )
    .unwrap();
    let record = execute(&plan, Options { checksum: true }, &false.into(), |_| {
        Decision::Skip
    });
    assert_eq!(record.completed_files(), 1, "{:?}", record.entries);
    let after = fs::metadata(destination.join(source.file_name().unwrap())).unwrap();
    assert_eq!(after.permissions().mode() & 0o777, 0o640);
    assert_eq!(after.modified().unwrap(), before.modified().unwrap());
}

#[cfg(target_os = "linux")]
#[test]
fn cross_filesystem_move_verifies_commits_and_removes_only_the_source() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let Some(destination) = common::second_filesystem(root.path()) else {
        return;
    };
    assert_ne!(
        fs::metadata(root.path()).unwrap().dev(),
        fs::metadata(destination.path()).unwrap().dev()
    );
    let source = root.path().join("source");
    fs::write(&source, "cross filesystem payload").unwrap();
    let plan = Plan::paths(
        Operation::Move,
        vec![source.clone()],
        Some(destination.path().into()),
    )
    .unwrap();
    let record = execute(&plan, Options { checksum: true }, &false.into(), |_| {
        Decision::Skip
    });
    assert_eq!(record.completed_files(), 1);
    assert!(!source.exists());
    assert_eq!(
        fs::read(destination.path().join("source")).unwrap(),
        b"cross filesystem payload"
    );
    assert_ne!(
        common::volume_root(root.path()),
        common::volume_root(destination.path())
    );
}

#[cfg(target_os = "linux")]
#[test]
fn exclusive_rename_refuses_cross_device_files_and_directories_without_a_fallback() {
    let root = tempfile::tempdir().unwrap();
    let Some(target) = common::second_filesystem(root.path()) else {
        return;
    };
    for directory in [false, true] {
        let source = root
            .path()
            .join(if directory { "directory" } else { "file" });
        let destination = target.path().join(source.file_name().unwrap());
        let payload = if directory {
            fs::create_dir(&source).unwrap();
            source.join("payload")
        } else {
            source.clone()
        };
        fs::write(&payload, "retained bytes").unwrap();
        let plan = Plan::paths(
            Operation::Rename,
            vec![source.clone()],
            Some(destination.clone()),
        )
        .unwrap();
        let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
        assert_eq!(record.entries[0].outcome, Outcome::Failed, "{record:?}");
        assert_eq!(
            terminal_file_manager::fs::inspect(&source).identity,
            plan.operands[0].identity
        );
        assert_eq!(fs::read(payload).unwrap(), b"retained bytes");
        assert!(!destination.exists());
    }
}

#[test]
fn cancelled_queued_plan_performs_no_filesystem_changes() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(root.path().join("source"), "original").unwrap();
    let plan = Plan::paths(
        Operation::Move,
        vec![root.path().join("source")],
        Some(destination.clone()),
    )
    .unwrap();
    let record = execute(&plan, Options::default(), &AtomicBool::new(true), |_| {
        panic!("Cancelled job emitted filesystem progress")
    });
    assert_eq!(record.entries[0].outcome, Outcome::Cancelled);
    assert_eq!(fs::read(root.path().join("source")).unwrap(), b"original");
    assert_eq!(fs::read_dir(destination).unwrap().count(), 0);
}

#[test]
fn unavailable_stage_parent_preserves_source_and_previous_destination() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), "original").unwrap();
    fs::write(root.path().join("not-directory"), "sentinel").unwrap();
    let plan = Plan::paths(
        Operation::Move,
        vec![root.path().join("source")],
        Some(root.path().join("not-directory")),
    )
    .unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| {
        Decision::Replace
    });
    assert_eq!(record.completed_files(), 0);
    assert_eq!(fs::read(root.path().join("source")).unwrap(), b"original");
    assert_eq!(
        fs::read(root.path().join("not-directory")).unwrap(),
        b"sentinel"
    );
}

#[test]
fn same_size_time_external_content_change_is_an_explicit_metadata_limit() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("destination")).unwrap();
    let source = root.path().join("source");
    fs::write(&source, "old bytes").unwrap();
    let timestamp = fs::metadata(&source).unwrap().modified().unwrap();
    let plan = Plan::paths(
        Operation::Copy,
        vec![source.clone()],
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
            fs::write(&source, "new bytes").unwrap();
            fs::OpenOptions::new()
                .write(true)
                .open(&source)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(timestamp))
                .unwrap();
        }
        Decision::Skip
    });
    assert_eq!(record.completed_files(), 1);
    assert!(!record.entries[0].checksum_verified);
    assert_eq!(
        fs::read(root.path().join("destination/source")).unwrap(),
        b"old bytes"
    );
    assert_eq!(fs::read(source).unwrap(), b"new bytes");
}
