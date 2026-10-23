use std::fs;
use terminal_file_manager::operations::{Decision, Operation, Options, Outcome, Plan, execute};

#[cfg(unix)]
#[test]
fn owned_unreadable_file_and_directory_modes_can_be_restored_without_read_access() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    assert!(
        !nix::unistd::geteuid().is_root(),
        "Run the owner-permission regression without root privileges"
    );
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    let directory = root.path().join("directory");
    fs::write(&file, "retained bytes").unwrap();
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("child"), "child bytes").unwrap();
    let mut results = Vec::new();
    for (path, mode) in [(&file, 0o600), (&directory, 0o700)] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
        let before = fs::metadata(path).unwrap();
        let plan = Plan::paths(Operation::Permissions(mode), vec![path.clone()], None).unwrap();
        let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
        let after = fs::metadata(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
        results.push((
            record.entries[0].outcome,
            after.permissions().mode() & 0o777,
            mode,
        ));
        assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
    }
    assert_eq!(fs::read(file).unwrap(), b"retained bytes");
    assert_eq!(fs::read(directory.join("child")).unwrap(), b"child bytes");
    assert!(results.iter().all(|(outcome, actual, expected)| *outcome == Outcome::Completed && actual == expected), "{results:?}");
}

#[cfg(unix)]
#[test]
fn chmod_refuses_a_replaced_regular_file_and_preserves_both_objects() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::write(&source, "original").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o400)).unwrap();
    let plan = Plan::paths(Operation::Permissions(0o600), vec![source.clone()], None).unwrap();
    let retained = root.path().join("retained");
    fs::rename(&source, &retained).unwrap();
    fs::write(&source, "replacement").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert_eq!(record.entries[0].outcome, Outcome::Changed);
    assert_eq!(
        fs::metadata(&source).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(
        fs::metadata(&retained).unwrap().permissions().mode() & 0o777,
        0o400
    );
    assert_eq!(fs::read(source).unwrap(), b"replacement");
    assert_eq!(fs::read(retained).unwrap(), b"original");
}

#[cfg(unix)]
#[test]
fn chmod_denied_parent_search_preserves_the_selected_object() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    assert!(!nix::unistd::geteuid().is_root());
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("parent");
    fs::create_dir(&parent).unwrap();
    let source = parent.join("source");
    fs::write(&source, "untouched").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o400)).unwrap();
    let before = fs::metadata(&source).unwrap();
    let plan = Plan::paths(Operation::Permissions(0o600), vec![source.clone()], None).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o000)).unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    let after = fs::metadata(&source).unwrap();
    assert_ne!(record.entries[0].outcome, Outcome::Completed);
    assert_eq!(after.permissions().mode() & 0o777, 0o400);
    assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
    assert_eq!(fs::read(source).unwrap(), b"untouched");
}

#[test]
fn mkdir_and_rename_never_replace_an_unapproved_entry() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("new directory");
    let plan = Plan::mkdir(directory.clone());
    assert_eq!(
        execute(&plan, Options::default(), &false.into(), |_| Decision::Skip)
            .completed_directories(),
        1
    );
    fs::write(directory.join("source"), "original").unwrap();
    let destination = directory.join("renamed");
    let rename = Plan::paths(
        Operation::Rename,
        vec![directory.join("source")],
        Some(destination.clone()),
    )
    .unwrap();
    let record = execute(&rename, Options::default(), &false.into(), |_| {
        Decision::Skip
    });
    assert_eq!(record.completed_files(), 1);
    assert_eq!(fs::read(&destination).unwrap(), b"original");
    assert!(!directory.join("source").exists());
    let record = execute(&plan, Options::default(), &false.into(), |_| {
        Decision::Replace
    });
    assert_eq!(record.entries[0].outcome, Outcome::Failed);
    assert_eq!(fs::read(destination).unwrap(), b"original");
}

#[cfg(unix)]
#[test]
fn permanent_delete_removes_links_themselves_and_counts_completed_descendants() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("selected/nested")).unwrap();
    fs::write(root.path().join("selected/nested/file"), "inside").unwrap();
    fs::write(root.path().join("outside"), "sentinel").unwrap();
    std::os::unix::fs::symlink(
        root.path().join("outside"),
        root.path().join("selected/link"),
    )
    .unwrap();
    let plan = Plan::paths(Operation::Delete, vec![root.path().join("selected")], None).unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert!(!root.path().join("selected").exists());
    assert_eq!(fs::read(root.path().join("outside")).unwrap(), b"sentinel");
    assert_eq!(record.completed_files(), 2);
    assert_eq!(record.completed_directories(), 2);
}

#[cfg(unix)]
#[test]
fn mode_changes_use_frozen_identity_and_reject_replacement_links() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::write(&source, "source").unwrap();
    let plan = Plan::paths(Operation::Permissions(0o640), vec![source.clone()], None).unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert_eq!(record.completed_files(), 1);
    assert_eq!(
        fs::metadata(&source).unwrap().permissions().mode() & 0o777,
        0o640
    );
    fs::write(root.path().join("outside"), "sentinel").unwrap();
    fs::rename(&source, root.path().join("retained-source")).unwrap();
    std::os::unix::fs::symlink("outside", &source).unwrap();
    let before = fs::metadata(root.path().join("outside"))
        .unwrap()
        .permissions()
        .mode();
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert_eq!(record.entries[0].outcome, Outcome::Changed);
    assert_eq!(
        fs::metadata(root.path().join("outside"))
            .unwrap()
            .permissions()
            .mode(),
        before
    );
}
