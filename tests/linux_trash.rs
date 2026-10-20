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
    assert!(
        LinuxTrash::new(separate.path().into())
            .put(&terminal_file_manager::fs::inspect(&source))
            .is_err()
    );
    assert_eq!(fs::read(source).unwrap(), b"preserved");
}
