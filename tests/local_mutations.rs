use std::fs;
use terminal_file_manager::operations::{Decision, Operation, Options, Outcome, Plan, execute};

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
