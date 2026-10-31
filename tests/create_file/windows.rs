use super::*;

#[test]
fn windows_nonreserved_device_like_names_create_regular_files() {
    for name in ["CLOCK$", "COM0", "LPT0", "CONIN$.txt", "CONOUT$.txt"] {
        let (root, mut c) = loaded();
        let native = root.path().join(name);
        drop(fs::File::create_new(&native).unwrap());
        assert!(fs::symlink_metadata(&native).unwrap().is_file(), "{name}");
        fs::remove_file(&native).unwrap();
        let plan = submit(&mut c, name);
        finish(&mut c, &plan);
        assert_eq!(
            c.app.operations.history.last().unwrap().state,
            terminal_file_manager::operations::JobState::Completed,
            "{name}"
        );
        assert!(fs::symlink_metadata(native).unwrap().is_file(), "{name}");
    }
}
