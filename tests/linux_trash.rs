#![cfg(target_os = "linux")]
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
    let source_root = tempfile::tempdir_in("/dev/shm").unwrap();
    assert_ne!(
        data.path().metadata().unwrap().dev(),
        source_root.path().metadata().unwrap().dev()
    );
    let source = source_root.path().join("owned-file");
    fs::write(&source, "recoverable volume fixture").unwrap();
    let alias = data.path().join("alias");
    symlink(source_root.path(), &alias).unwrap();
    let entry = terminal_file_manager::fs::inspect(&alias.join("owned-file"));
    let trash = LinuxTrash::new(data.path().into());
    let receipt = trash.put(&entry).unwrap();
    assert!(!source.exists());
    assert_eq!(
        fs::read(&receipt.path).unwrap(),
        b"recoverable volume fixture"
    );
    let info = fs::read_to_string(&receipt.info).unwrap();
    let relative = source.strip_prefix("/dev/shm").unwrap();
    assert!(info.contains(&format!("Path={}\n", relative.display())));
    assert_eq!(
        entry.identity.unwrap().object,
        terminal_file_manager::fs::inspect(&receipt.path)
            .identity
            .unwrap()
            .object
    );
    fs::rename(&receipt.path, &source).unwrap();
    fs::remove_file(receipt.info).unwrap();
    let link_receipt = trash
        .put(&terminal_file_manager::fs::inspect(&alias))
        .unwrap();
    assert!(
        link_receipt
            .path
            .starts_with(data.path().join("Trash/files"))
    );
    assert_eq!(fs::read(&source).unwrap(), b"recoverable volume fixture");
    assert_eq!(
        fs::read_link(&link_receipt.path).unwrap(),
        source_root.path()
    );
    fs::rename(link_receipt.path, &alias).unwrap();
    fs::remove_file(link_receipt.info).unwrap();
}
