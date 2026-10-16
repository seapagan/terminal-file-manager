//! Native read-only listing contracts, adapted from lsplus's filesystem cases.

use std::{fs, sync::atomic::AtomicBool};
use terminal_file_manager::fs::{EntryKind, IgnoreContext, list};
use terminal_file_manager::listing::{Grouping, Sort, SortPolicy};

#[test]
fn natural_default_keeps_directories_first_and_reverse_is_independent() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("file10"), "ten").unwrap();
    fs::write(root.path().join("file2"), "two").unwrap();
    fs::create_dir(root.path().join("zdir")).unwrap();
    let mut snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    SortPolicy::default().apply(&mut snapshot.entries);
    assert_eq!(
        snapshot
            .entries
            .iter()
            .map(|e| e.name.to_str().unwrap())
            .collect::<Vec<_>>(),
        ["zdir", "file2", "file10"]
    );
    SortPolicy {
        reverse: true,
        ..SortPolicy::default()
    }
    .apply(&mut snapshot.entries);
    assert_eq!(snapshot.entries[0].kind, EntryKind::Directory);
    assert_eq!(snapshot.entries[1].name, "file10");
    SortPolicy {
        sort: Sort::Size,
        grouping: Grouping::Mixed,
        reverse: false,
    }
    .apply(&mut snapshot.entries);
    assert_eq!(fs::read(root.path().join("file2")).unwrap(), b"two");
}

#[test]
fn invalid_directory_is_an_error_without_fallback() {
    let root = tempfile::tempdir().unwrap();
    assert!(
        list(
            &root.path().join("missing"),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false)
        )
        .is_err()
    );
}

#[test]
fn ignore_changes_are_observed_on_refresh() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    fs::write(root.path().join(".gitignore"), "*.log\n").unwrap();
    fs::write(root.path().join("a.log"), "data").unwrap();
    let context = IgnoreContext::isolated();
    let snapshot = list(root.path(), &context, &AtomicBool::new(false)).unwrap();
    assert!(
        snapshot
            .entries
            .iter()
            .find(|e| e.name == "a.log")
            .unwrap()
            .ignored
            .unwrap()
    );
    fs::write(root.path().join(".gitignore"), "!*.log\n").unwrap();
    let refreshed = list(root.path(), &context, &AtomicBool::new(false)).unwrap();
    assert!(
        !refreshed
            .entries
            .iter()
            .find(|e| e.name == "a.log")
            .unwrap()
            .ignored
            .unwrap()
    );
}

#[test]
fn nested_global_and_linked_worktree_rules_keep_precedence() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    let git = root.path().join("common");
    let worktree = git.join("worktrees/one");
    fs::create_dir_all(repo.join("nested")).unwrap();
    fs::create_dir_all(&worktree).unwrap();
    fs::create_dir_all(git.join("info")).unwrap();
    fs::write(
        repo.join(".git"),
        format!("gitdir: {}\n", worktree.display()),
    )
    .unwrap();
    fs::write(worktree.join("commondir"), "../..\n").unwrap();
    fs::write(git.join("info/exclude"), "excluded\n").unwrap();
    fs::write(repo.join(".gitignore"), "*.log\n").unwrap();
    fs::write(repo.join("nested/.gitignore"), "!keep.log\n").unwrap();
    let global = root.path().join("global");
    fs::write(&global, "global\n").unwrap();
    for name in ["a.log", "keep.log", "global", "excluded"] {
        fs::write(repo.join("nested").join(name), "data").unwrap();
    }
    let context = IgnoreContext {
        global_file: Some(global),
        discover_global: false,
    };
    let snapshot = list(&repo.join("nested"), &context, &AtomicBool::new(false)).unwrap();
    for (name, ignored) in [
        ("a.log", true),
        ("keep.log", false),
        ("global", true),
        ("excluded", true),
    ] {
        assert_eq!(
            snapshot
                .entries
                .iter()
                .find(|e| e.name == name)
                .unwrap()
                .ignored,
            Some(ignored),
            "{name}"
        );
    }
}

#[test]
fn malformed_rules_are_unknown_and_cancellation_is_not_partial_success() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    fs::write(root.path().join(".gitignore"), "{a,b\n").unwrap();
    let snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(!snapshot.errors.is_empty());
    assert!(snapshot.entries.iter().all(|e| e.ignored.is_none()));
    let error = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(true),
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
}

#[test]
fn nested_whitelist_cannot_reinclude_a_child_of_an_excluded_parent() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    fs::create_dir(root.path().join("ignored-dir")).unwrap();
    fs::write(root.path().join(".gitignore"), "ignored-dir/\n").unwrap();
    fs::write(root.path().join("ignored-dir/.gitignore"), "!keep\n").unwrap();
    fs::write(root.path().join("ignored-dir/keep"), "data").unwrap();
    let snapshot = list(
        &root.path().join("ignored-dir"),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        snapshot
            .entries
            .iter()
            .find(|e| e.name == "keep")
            .unwrap()
            .ignored,
        Some(true)
    );
}

#[cfg(unix)]
#[test]
fn unix_links_modes_and_identity_are_preserved() {
    use std::ffi::OsString;
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = tempfile::tempdir().unwrap();
    let raw = OsString::from("a界");
    fs::write(root.path().join(&raw), "bytes").unwrap();
    fs::set_permissions(root.path().join(&raw), fs::Permissions::from_mode(0o751)).unwrap();
    symlink("missing", root.path().join("broken")).unwrap();
    symlink(".", root.path().join("cycle")).unwrap();
    let snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let entry = snapshot.entries.iter().find(|e| e.name == raw).unwrap();
    assert_eq!(entry.path, root.path().join(&raw));
    assert!(entry.executable);
    assert!(entry.identity.is_some());
    let broken = snapshot
        .entries
        .iter()
        .find(|e| e.name == "broken")
        .unwrap();
    assert_eq!(broken.kind, EntryKind::Link);
    assert!(broken.error.is_some());
    let cycle = snapshot.entries.iter().find(|e| e.name == "cycle").unwrap();
    assert!(cycle.enterable);
    assert!(!cycle.may_recurse());
}

#[cfg(target_os = "linux")]
#[test]
fn linux_invalid_utf8_names_remain_distinct_native_paths() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempfile::tempdir().unwrap();
    let names = [
        OsString::from_vec(vec![b'a', 0xfe]),
        OsString::from_vec(vec![b'a', 0xff]),
    ];
    for name in &names {
        fs::write(root.path().join(name), b"raw").unwrap();
    }
    let snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    for name in names {
        assert!(
            snapshot
                .entries
                .iter()
                .any(|e| e.path == root.path().join(&name))
        );
    }
    assert_ne!(snapshot.entries[0].path, snapshot.entries[1].path);
}
