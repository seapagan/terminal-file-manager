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
fn broken_descendants_report_incomplete_totals_without_losing_regular_files() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("one"), b"123").unwrap();
    let broken = root.path().join("broken");
    std::os::unix::fs::symlink("missing", &broken).unwrap();
    let result = totals(root.path(), &AtomicBool::new(false)).unwrap();
    assert_eq!((result.files, result.directories, result.bytes), (1, 0, 3));
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains(&broken.display().to_string()));
    assert_eq!(
        fs::read_link(&broken).unwrap(),
        std::path::Path::new("missing")
    );
    assert_eq!(fs::read(root.path().join("one")).unwrap(), b"123");
}
