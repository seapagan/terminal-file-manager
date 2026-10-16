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
