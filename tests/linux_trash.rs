#![cfg(target_os = "linux")]
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
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
fn unsafe_permissions_and_cross_device_targets_fail_without_fallback() {
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
    let separate = tempfile::tempdir_in("/dev/shm").unwrap();
    assert_ne!(
        root.path().metadata().unwrap().dev(),
        separate.path().metadata().unwrap().dev()
    );
    let error = LinuxTrash::new(separate.path().into())
        .put(&terminal_file_manager::fs::inspect(&source))
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Cross-filesystem Trash unsupported")
    );
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
