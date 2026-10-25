use super::*;
use std::{os::windows::fs::OpenOptionsExt, sync::atomic::Ordering};

fn root() -> (tempfile::TempDir, std::path::PathBuf) {
    let owner = tempfile::tempdir().unwrap();
    let path = fs::canonicalize(owner.path()).unwrap();
    let file = native::open_directory(&path).unwrap();
    assert!(
        native::query_directory_volume(&file).unwrap().is_some(),
        "native acceptance requires a local fixed NTFS fixture"
    );
    (owner, path)
}

fn parity(path: &Path, context: &super::super::IgnoreContext) {
    let cancel = AtomicBool::new(false);
    let fast = super::super::list_backend(path, context, &cancel, true).unwrap();
    let before = super::super::list_backend(path, context, &cancel, false).unwrap();
    assert_eq!(fast.path, before.path);
    assert_eq!(fast.errors, before.errors);
    assert_eq!(fast.directory, before.directory);
    assert_eq!(
        fast.entries, before.entries,
        "complete entries, including native enumeration order"
    );
    let initial = super::super::directory_identity(path, &mut crate::owner::Cache::default());
    assert!(list(path, initial.as_ref(), &cancel).unwrap().is_some());
}

#[test]
fn hard_link_changes_through_another_spelling_match_fresh_inspection() {
    let (_owner, path) = root();
    let listed = path.join("listed");
    fs::create_dir(&listed).unwrap();
    let original = path.join("outside-spelling");
    fs::write(&original, "old").unwrap();
    fs::hard_link(&original, listed.join("alias")).unwrap();
    fs::write(&original, "changed-length-through-another-link").unwrap();
    parity(&listed, &super::super::IgnoreContext::isolated());
}

#[test]
fn shared_open_writer_metadata_matches_fresh_inspection() {
    use std::io::Write;
    let (_owner, path) = root();
    let original = path.join("writer");
    fs::write(&original, "old").unwrap();
    let mut writer = fs::OpenOptions::new()
        .append(true)
        .share_mode(7)
        .open(&original)
        .unwrap();
    writer
        .write_all(b"-updated-while-handle-stays-open")
        .unwrap();
    writer.flush().unwrap();
    parity(&path, &super::super::IgnoreContext::isolated());
}

#[test]
fn complete_plain_and_nested_ignore_snapshots_match_on_large_ntfs_directories() {
    let (_owner, path) = root();
    fs::create_dir(path.join(".git")).unwrap();
    fs::create_dir_all(path.join("a/b")).unwrap();
    fs::write(path.join(".gitignore"), "*.log\na/ignored*\n").unwrap();
    fs::write(path.join("a/.gitignore"), "*.tmp\n").unwrap();
    fs::write(path.join("a/b/.gitignore"), "!keep.log\n").unwrap();
    let context = super::super::IgnoreContext::isolated();
    for size in [2000u32, 10000] {
        let directory = path.join(format!("plain-{size}"));
        fs::create_dir(&directory).unwrap();
        for index in 0..size {
            fs::write(
                directory.join(format!("item{index:05}.log")),
                index.to_le_bytes(),
            )
            .unwrap();
        }
        parity(&directory, &context);
        let nested = path.join("a/b").join(format!("nested-{size}"));
        fs::rename(&directory, &nested).unwrap();
        fs::write(nested.join(".gitignore"), "item*2.log\n!item00002.log\n").unwrap();
        parity(&nested, &context);
    }
}

#[test]
fn raw_names_links_and_attributes_preserve_every_entry_field() {
    let (_owner, path) = root();
    for name in [
        OsString::from("emoji-😀-e\u{301}"),
        OsString::from_wide(&[0xd800]),
        OsString::from("trailing. "),
    ] {
        fs::write(path.join(name), "owned").unwrap();
    }
    let file = path.join("ordinary");
    fs::write(&file, "metadata").unwrap();
    let original_permissions = fs::metadata(&file).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&file, permissions).unwrap();
    fs::create_dir(path.join("directory")).unwrap();
    let hidden = std::process::Command::new("attrib")
        .arg("+H")
        .arg(&file)
        .output()
        .unwrap();
    assert!(hidden.status.success(), "{hidden:?}");
    std::os::windows::fs::symlink_file(&file, path.join("file-link")).unwrap();
    std::os::windows::fs::symlink_dir(path.join("directory"), path.join("dir-link")).unwrap();
    std::os::windows::fs::symlink_file(path.join("missing"), path.join("broken")).unwrap();
    parity(&path, &super::super::IgnoreContext::isolated());
    sort_parity(&path);
    fs::set_permissions(file, original_permissions).unwrap();
}

fn sort_parity(path: &Path) {
    use crate::listing::{Grouping, Sort, SortPolicy};
    let context = super::super::IgnoreContext::isolated();
    let cancel = AtomicBool::new(false);
    let before = super::super::list_backend(path, &context, &cancel, false).unwrap();
    let after = super::super::list_backend(path, &context, &cancel, true).unwrap();
    for sort in [
        Sort::Natural,
        Sort::Name,
        Sort::Extension,
        Sort::Size,
        Sort::Modified,
        Sort::Created,
        Sort::Kind,
        Sort::Version,
        Sort::Unsorted,
    ] {
        for grouping in [Grouping::First, Grouping::Last, Grouping::Mixed] {
            for reverse in [false, true] {
                let policy = SortPolicy {
                    sort,
                    grouping,
                    reverse,
                };
                let mut expected = before.entries.clone();
                let mut actual = after.entries.clone();
                policy.apply(&mut expected);
                policy.apply(&mut actual);
                assert_eq!(actual, expected, "{policy:?}");
            }
        }
    }
}

#[test]
fn actual_denied_directory_keeps_native_error_without_fallback_success() {
    let (_owner, path) = root();
    let directory = path.join("denied");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("owned"), "owned").unwrap();
    let user = std::env::var_os("USERNAME").unwrap();
    let deny = std::process::Command::new("icacls")
        .arg(&directory)
        .arg("/deny")
        .arg(format!("{}:(RD)", user.to_str().unwrap()))
        .output()
        .unwrap();
    let cancel = AtomicBool::new(false);
    let context = super::super::IgnoreContext::isolated();
    let baseline = super::super::list_backend(&directory, &context, &cancel, false);
    let buffered = super::super::list_backend(&directory, &context, &cancel, true);
    let restored = std::process::Command::new("icacls")
        .arg(&directory)
        .arg("/remove:d")
        .arg(user)
        .output()
        .unwrap();
    assert!(
        restored.status.success(),
        "owned ACL restoration failed: {restored:?}"
    );
    assert!(deny.status.success(), "{deny:?}");
    assert_eq!(baseline.unwrap_err().raw_os_error(), Some(5));
    assert_eq!(buffered.unwrap_err().raw_os_error(), Some(5));
}

#[test]
fn case_sensitive_native_names_remain_separate() {
    let (_owner, path) = root();
    let result = std::process::Command::new("fsutil")
        .args(["file", "setCaseSensitiveInfo"])
        .arg(&path)
        .arg("enable")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "native case-sensitive fixture failed: {result:?}"
    );
    fs::write(path.join("Name"), "first").unwrap();
    fs::write(path.join("name"), "second").unwrap();
    assert_eq!(fs::read_dir(&path).unwrap().count(), 2);
    parity(&path, &super::super::IgnoreContext::isolated());
}

#[test]
fn reparse_aliases_use_original_backend_and_entry_hints_never_invent_metadata() {
    let (_owner, path) = root();
    let target = path.join("target");
    let link = path.join("alias");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("owned"), "data").unwrap();
    std::os::windows::fs::symlink_dir(&target, &link).unwrap();
    let initial = super::super::directory_identity(&link, &mut crate::owner::Cache::default());
    assert!(
        list(&link, initial.as_ref(), &AtomicBool::new(false))
            .unwrap()
            .is_none()
    );
    let context = super::super::IgnoreContext::isolated();
    let cancel = AtomicBool::new(false);
    assert_eq!(
        super::super::list_backend(&link, &context, &cancel, false)
            .unwrap()
            .entries,
        super::super::list_backend(&link, &context, &cancel, true)
            .unwrap()
            .entries
    );
    let row = Record {
        name: "owned".encode_utf16().collect(),
        attributes: 0x400,
        length: 123,
        created: 1,
        accessed: 2,
        modified: 3,
        id: 456,
    };
    assert_eq!(
        entry(&target, 999, row),
        super::super::inspect(&target.join("owned"))
    );
}

#[test]
fn transient_batch_handles_release_current_parent_and_grandparent_renames() {
    let (_owner, path) = root();
    let current = path.join("grand/parent/current");
    fs::create_dir_all(&current).unwrap();
    fs::write(current.join("owned"), "data").unwrap();
    let file = native::open_directory(&current).unwrap();
    let mut buffer = native::DirectoryBuffer([0; 65536]);
    assert!(native::read_directory_batch(&file, &mut buffer).unwrap());
    for target in [
        current.clone(),
        current.parent().unwrap().to_owned(),
        current.parent().unwrap().parent().unwrap().to_owned(),
    ] {
        let moved = target.with_extension("moved");
        let result = fs::rename(&target, &moved);
        eprintln!("held buffered enumeration rename {target:?}: {result:?}");
        if result.is_ok() {
            fs::rename(&moved, &target).unwrap();
        }
    }
    drop(file);
    for target in [
        current.clone(),
        current.parent().unwrap().to_owned(),
        current.parent().unwrap().parent().unwrap().to_owned(),
    ] {
        let moved = target.with_extension("closed");
        fs::rename(&target, &moved).unwrap();
        fs::rename(moved, target).unwrap();
    }
}

#[test]
fn long_native_path_sizes_final_path_buffer_without_truncation() {
    let (_owner, path) = root();
    let mut long = path;
    for _ in 0..8 {
        long.push("native-path-component-0123456789");
    }
    fs::create_dir_all(&long).unwrap();
    fs::write(long.join("long-entry"), "owned").unwrap();
    assert!(long.as_os_str().len() > 261);
    parity(&long, &super::super::IgnoreContext::isolated());
}

#[test]
fn exclusive_open_entry_does_not_erase_existing_inspection_errors() {
    let (_owner, path) = root();
    let name = path.join("locked");
    fs::write(&name, "owned").unwrap();
    let _held = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(name)
        .unwrap();
    parity(&path, &super::super::IgnoreContext::isolated());
}

#[test]
fn cancelled_or_replaced_directory_never_returns_trusted_records() {
    let (_owner, path) = root();
    let directory = path.join("selected");
    fs::create_dir(&directory).unwrap();
    let initial = super::super::directory_identity(&directory, &mut crate::owner::Cache::default());
    fs::rename(&directory, path.join("retained")).unwrap();
    fs::create_dir(&directory).unwrap();
    let cancel = AtomicBool::new(false);
    assert!(list(&directory, initial.as_ref(), &cancel).is_err());
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(
        list(&directory, initial.as_ref(), &cancel)
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::Interrupted
    );
}

#[test]
fn replacement_after_enumeration_cannot_install_a_verified_directory() {
    let (_owner, path) = root();
    let selected = path.join("selected");
    fs::create_dir(&selected).unwrap();
    fs::write(selected.join("old"), "original").unwrap();
    let initial = super::super::directory_identity(&selected, &mut crate::owner::Cache::default());
    let cancelled = AtomicBool::new(false);
    let listing = list(&selected, initial.as_ref(), &cancelled)
        .unwrap()
        .unwrap();
    fs::rename(&selected, path.join("retained")).unwrap();
    fs::create_dir(&selected).unwrap();
    fs::write(selected.join("new"), "replacement").unwrap();
    let snapshot = super::super::finish_buffered(
        &selected,
        &super::super::IgnoreContext::isolated(),
        &cancelled,
        initial,
        listing,
    )
    .unwrap();
    assert!(snapshot.directory.is_none());
    assert_eq!(snapshot.entries[0].name, "old");
    assert_eq!(fs::read(selected.join("new")).unwrap(), b"replacement");
}

#[test]
fn native_sdk_layout_and_error_classification_match_parser_contract() {
    use windows_sys::Win32::Storage::FileSystem::FILE_ID_BOTH_DIR_INFO as Row;
    assert_eq!(std::mem::offset_of!(Row, FileName), 104);
    assert_eq!(std::mem::offset_of!(Row, FileId), 96);
    assert_eq!(std::mem::offset_of!(Row, FileNameLength), 60);
    assert_eq!(std::mem::align_of::<native::DirectoryBuffer>(), 8);
    for code in [1, 50, 87] {
        assert!(native::unsupported(&io::Error::from_raw_os_error(code)));
    }
    for code in [2, 3, 5, 18, 32, 1117] {
        assert!(!native::unsupported(&io::Error::from_raw_os_error(code)));
    }
}
