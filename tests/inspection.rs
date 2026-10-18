//! Bounded, cancellable read-only directory inspection.

use std::{fs, sync::atomic::AtomicBool};
use terminal_file_manager::fs::totals;

#[test]
fn totals_count_files_and_directories_without_following_links() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("nested")).unwrap();
    fs::write(root.path().join("one"), b"123").unwrap();
    fs::write(root.path().join("nested/two"), b"45").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(".", root.path().join("cycle")).unwrap();
    let result = totals(root.path(), &AtomicBool::new(false)).unwrap();
    assert_eq!(result.files, 2);
    assert_eq!(result.directories, 1);
    assert_eq!(result.bytes, 5);
    assert!(result.errors.is_empty());
    assert_eq!(fs::read(root.path().join("one")).unwrap(), b"123");
}

#[test]
fn cancelled_totals_are_not_reported_complete() {
    let root = tempfile::tempdir().unwrap();
    let error = totals(root.path(), &AtomicBool::new(true)).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
}

#[test]
fn depth_limited_totals_preserve_observed_bytes_and_report_the_omitted_subtree() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("visible"), b"123").unwrap();
    let mut deepest = root.path().to_path_buf();
    for _ in 0..129 {
        deepest.push("d");
        fs::create_dir(&deepest).unwrap();
    }
    let sentinel = deepest.join("omitted");
    fs::write(&sentinel, b"12345").unwrap();
    let result = totals(root.path(), &AtomicBool::new(false)).unwrap();
    assert_eq!(
        (result.files, result.directories, result.bytes),
        (1, 128, 3)
    );
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains("Depth limit reached:"));
    assert!(result.errors[0].contains(&deepest.parent().unwrap().display().to_string()));
    assert_eq!(fs::read(sentinel).unwrap(), b"12345");
}

#[cfg(unix)]
#[test]
fn broken_links_do_not_make_no_follow_totals_incomplete() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("one"), b"123").unwrap();
    let broken = root.path().join("broken");
    std::os::unix::fs::symlink("missing", &broken).unwrap();
    let result = totals(root.path(), &AtomicBool::new(false)).unwrap();
    assert_eq!((result.files, result.directories, result.bytes), (1, 0, 3));
    assert!(result.errors.is_empty());
    assert!(terminal_file_manager::fs::inspect(&broken).error.is_some());
    assert_eq!(
        fs::read_link(&broken).unwrap(),
        std::path::Path::new("missing")
    );
    assert_eq!(fs::read(root.path().join("one")).unwrap(), b"123");
}

#[cfg(unix)]
#[test]
fn unreadable_descendants_remain_incomplete_without_following_links() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let denied = root.path().join("denied");
    fs::create_dir(&denied).unwrap();
    fs::write(denied.join("omitted"), b"secret").unwrap();
    fs::write(root.path().join("visible"), b"123").unwrap();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o0)).unwrap();
    let readable = fs::read_dir(&denied).is_ok();
    let result = totals(root.path(), &AtomicBool::new(false));
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o700)).unwrap();
    if readable {
        // Privileged hosts cannot establish a permission-denied traversal.
        assert_eq!(result.unwrap().bytes, 9);
    } else {
        let result = result.unwrap();
        assert_eq!((result.files, result.directories, result.bytes), (1, 1, 3));
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains(&denied.display().to_string()));
    }
}

#[cfg(unix)]
#[test]
fn totals_reject_link_roots_and_never_count_external_targets() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("excluded"), b"12345").unwrap();
    let link = root.path().join("outside");
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    let error = totals(&link, &AtomicBool::new(false)).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    let result = totals(root.path(), &AtomicBool::new(false)).unwrap();
    assert_eq!((result.files, result.directories, result.bytes), (0, 0, 0));
    assert!(result.errors.is_empty());
    assert_eq!(fs::read(outside.path().join("excluded")).unwrap(), b"12345");
}
