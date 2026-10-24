#![cfg(target_os = "linux")]
#[path = "volume_fixture.rs"]
mod common;
#[path = "trash_fixture.rs"]
mod fixtures;
use fixtures::{OwnedStore, ReceiptFixture};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
use terminal_file_manager::operations::trash::LinuxTrash;

#[test]
fn restricted_trash_keeps_links_and_reserves_collision_safe_metadata() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    fs::create_dir(&data).unwrap();
    let trash = LinuxTrash::new(data);
    fs::write(root.path().join("sentinel"), "unchanged").unwrap();
    for name in ["first", "second"] {
        let parent = root.path().join(name);
        fs::create_dir(&parent).unwrap();
        symlink("../sentinel", parent.join("link")).unwrap();
        let receipt = trash
            .put(&terminal_file_manager::fs::inspect(&parent.join("link")))
            .unwrap();
        assert_eq!(
            fs::read_link(&receipt.path).unwrap(),
            std::path::PathBuf::from("../sentinel")
        );
        assert!(fs::symlink_metadata(parent.join("link")).is_err());
        let info = fs::read_to_string(&receipt.info).unwrap();
        assert!(info.starts_with("[Trash Info]\nPath="));
        assert!(info.contains("\nDeletionDate="));
    }
    assert_eq!(
        fs::read(root.path().join("sentinel")).unwrap(),
        b"unchanged"
    );
    assert_eq!(
        fs::read_dir(root.path().join("data/Trash/files"))
            .unwrap()
            .count(),
        2
    );
}

#[test]
fn unsafe_permissions_and_relative_storage_fail_without_fallback() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    fs::create_dir_all(data.join("Trash/files")).unwrap();
    fs::set_permissions(data.join("Trash"), fs::Permissions::from_mode(0o777)).unwrap();
    let source = root.path().join("source");
    fs::write(&source, "preserved").unwrap();
    assert!(
        LinuxTrash::new(data)
            .put(&terminal_file_manager::fs::inspect(&source))
            .is_err()
    );
    assert_eq!(fs::read(&source).unwrap(), b"preserved");
    let error = LinuxTrash::new("relative-data".into())
        .put(&terminal_file_manager::fs::inspect(&source))
        .unwrap_err();
    assert!(error.to_string().contains("Relative XDG_DATA_HOME"));
    assert_eq!(fs::read(source).unwrap(), b"preserved");
}

#[test]
fn symlinked_parent_on_the_trash_filesystem_is_supported() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("parent");
    let alias = root.path().join("alias");
    fs::create_dir(&parent).unwrap();
    fs::write(parent.join("source"), "preserved").unwrap();
    symlink(&parent, &alias).unwrap();
    let source = alias.join("source");
    let receipt = LinuxTrash::new(root.path().join("data"))
        .put(&terminal_file_manager::fs::inspect(&source))
        .unwrap();
    assert_eq!(fs::read(receipt.path).unwrap(), b"preserved");
    assert!(!source.exists());
    assert!(alias.is_symlink());
}

#[test]
fn real_separate_volume_preserves_payload_and_relative_recovery_metadata() {
    use std::os::unix::fs::MetadataExt;
    let data = tempfile::tempdir().unwrap();
    let Some(source_root) = common::second_filesystem(data.path()) else {
        return;
    };
    assert_ne!(
        data.path().metadata().unwrap().dev(),
        source_root.path().metadata().unwrap().dev()
    );
    let source = source_root.path().join("owned-file");
    let top = common::volume_root(source_root.path());
    let storage = OwnedStore::prepare(&top).unwrap();
    fs::write(&source, "recoverable volume fixture").unwrap();
    let alias = data.path().join("alias");
    symlink(source_root.path(), &alias).unwrap();
    let entry = terminal_file_manager::fs::inspect(&alias.join("owned-file"));
    let trash = LinuxTrash::new(data.path().into());
    let fixture = ReceiptFixture::new(trash.put(&entry).unwrap(), source.clone());
    let receipt = &fixture.receipt;
    assert!(!source.exists());
    assert_eq!(
        fs::read(&receipt.path).unwrap(),
        b"recoverable volume fixture"
    );
    let info = fs::read_to_string(&receipt.info).unwrap();
    let physical = fs::canonicalize(source_root.path())
        .unwrap()
        .join("owned-file");
    let relative = physical.strip_prefix(&top).unwrap();
    assert!(info.contains(&format!("Path={}\n", relative.display())));
    assert_eq!(
        entry.identity.unwrap().object,
        terminal_file_manager::fs::inspect(&receipt.path)
            .identity
            .unwrap()
            .object
    );
    fixture.cleanup().unwrap();
    assert!(!receipt.path.exists() && !receipt.info.exists());
    recycle_alias(&trash, &alias, &source, source_root.path(), data.path());
    storage.cleanup().unwrap();
}

fn recycle_alias(
    trash: &LinuxTrash,
    alias: &std::path::Path,
    source: &std::path::Path,
    source_root: &std::path::Path,
    data: &std::path::Path,
) {
    let link_fixture = ReceiptFixture::new(
        trash
            .put(&terminal_file_manager::fs::inspect(alias))
            .unwrap(),
        alias.to_owned(),
    );
    let link_receipt = &link_fixture.receipt;
    assert!(link_receipt.path.starts_with(data.join("Trash/files")));
    assert_eq!(fs::read(source).unwrap(), b"recoverable volume fixture");
    assert_eq!(fs::read_link(&link_receipt.path).unwrap(), source_root);
    link_fixture.cleanup().unwrap();
    assert!(!link_receipt.path.exists() && !link_receipt.info.exists());
}

#[test]
fn receipt_cleanup_keeps_a_racing_restore_target_and_replaced_payload() {
    for replace_payload in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let payload = root.path().join("payload");
        let info = root.path().join("fixture.trashinfo");
        let original = root.path().join("original");
        fs::write(&payload, "owned bytes").unwrap();
        fs::write(&info, "owned receipt").unwrap();
        let fixture = ReceiptFixture::new(
            terminal_file_manager::operations::trash::LinuxReceipt {
                path: payload.clone(),
                info: info.clone(),
            },
            original.clone(),
        );
        if replace_payload {
            fs::rename(&payload, root.path().join("held-owned")).unwrap();
            fs::write(&payload, "foreign payload").unwrap();
        } else {
            fs::write(&original, "foreign restore target").unwrap();
        }
        assert!(fixture.cleanup().is_err());
        assert_eq!(fs::read(&info).unwrap(), b"owned receipt");
        if replace_payload {
            assert_eq!(fs::read(&payload).unwrap(), b"foreign payload");
        } else {
            assert_eq!(fs::read(&original).unwrap(), b"foreign restore target");
        }
    }
}

#[test]
fn volume_fixture_cleanup_removes_only_created_empty_directories() {
    let root = tempfile::tempdir().unwrap();
    let prepared = OwnedStore::prepare(root.path()).unwrap();
    prepared.cleanup().unwrap();
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    let existing = root.path().join("existing");
    let owned = root.path().join("owned");
    fs::create_dir(&existing).unwrap();
    let mut storage = OwnedStore::default();
    storage.create(&existing).unwrap();
    storage.create(&owned).unwrap();
    fs::write(owned.join("unrelated"), "preserved").unwrap();
    assert!(storage.cleanup().is_err());
    assert_eq!(fs::read(owned.join("unrelated")).unwrap(), b"preserved");
    fs::remove_file(owned.join("unrelated")).unwrap();
    storage.cleanup().unwrap();
    assert!(!owned.exists());
    assert!(existing.exists());
}

#[test]
fn volume_fixture_cleanup_refuses_a_replacement_even_when_empty_or_dangling() {
    for link in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("owned");
        let held = root.path().join("held-original");
        let mut storage = OwnedStore::default();
        storage.create(&path).unwrap();
        fs::rename(&path, &held).unwrap();
        if link {
            symlink("missing", &path).unwrap();
        } else {
            fs::create_dir(&path).unwrap();
        }
        let replacement = terminal_file_manager::fs::inspect(&path).identity;
        assert!(storage.cleanup().is_err());
        assert_eq!(
            terminal_file_manager::fs::inspect(&path).identity,
            replacement
        );
        assert!(held.is_dir());
    }
}
