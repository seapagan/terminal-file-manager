use std::fs;
use terminal_file_manager::operations::{
    Decision, Event, Operation, Options, Outcome, Plan, Stage, execute,
};

#[cfg(windows)]
#[test]
fn old_windows_rename_primitive_reproduces_the_overwrite_in_an_owned_fixture() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::write(&source, "incoming").unwrap();
    fs::write(&target, "racing independent object").unwrap();
    let before = terminal_file_manager::fs::inspect(&target)
        .identity
        .unwrap()
        .object;
    fs::rename(source, &target).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"incoming");
    assert_ne!(
        terminal_file_manager::fs::inspect(&target)
            .identity
            .unwrap()
            .object,
        before
    );
}

fn make(path: &std::path::Path, directory: bool, bytes: &[u8]) {
    if directory {
        fs::create_dir(path).unwrap();
        fs::write(path.join("payload"), bytes).unwrap();
    } else {
        fs::write(path, bytes).unwrap();
    }
}

fn bytes(path: &std::path::Path, directory: bool) -> Vec<u8> {
    fs::read(if directory {
        path.join("payload")
    } else {
        path.into()
    })
    .unwrap()
}

#[test]
fn a_racing_rename_destination_keeps_its_bytes_and_identity() {
    for directory in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("selected-雪");
        let target = root.path().join("renamed-λ");
        make(&source, directory, b"selected bytes");
        let plan = Plan::paths(
            Operation::Rename,
            vec![source.clone()],
            Some(target.clone()),
        )
        .unwrap();
        let mut collision = None;
        let record = execute(&plan, Options::default(), &false.into(), |event| {
            if matches!(
                event,
                Event::Progress {
                    stage: Stage::Verified,
                    ..
                }
            ) {
                make(&target, directory, b"independent bytes");
                collision = terminal_file_manager::fs::inspect(&target).identity;
            }
            Decision::Skip
        });
        assert!(collision.is_some(), "native boundary must be exercised");
        assert_ne!(record.entries[0].outcome, Outcome::Completed, "{record:?}");
        assert_eq!(bytes(&source, directory), b"selected bytes");
        assert_eq!(bytes(&target, directory), b"independent bytes");
        assert_eq!(
            terminal_file_manager::fs::inspect(&source).identity,
            plan.operands[0].identity
        );
        assert_eq!(
            terminal_file_manager::fs::inspect(&target).identity,
            collision
        );
    }
}

#[test]
fn native_exclusive_rename_preserves_files_and_nonempty_directories() {
    for directory in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("selected-雪");
        let target = root.path().join("renamed-λ");
        make(&source, directory, b"selected bytes");
        let plan = Plan::paths(
            Operation::Rename,
            vec![source.clone()],
            Some(target.clone()),
        )
        .unwrap();
        let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
        assert_eq!(record.entries[0].outcome, Outcome::Completed, "{record:?}");
        assert!(!source.exists());
        assert_eq!(bytes(&target, directory), b"selected bytes");
        assert_eq!(
            terminal_file_manager::fs::inspect(&target)
                .identity
                .unwrap()
                .object,
            plan.operands[0].identity.as_ref().unwrap().object
        );
    }
}

#[test]
fn exclusive_rename_moves_only_the_selected_symlink() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("link");
    let target = root.path().join("renamed");
    fs::write(root.path().join("sentinel"), b"unchanged").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("sentinel", &source).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file("sentinel", &source).unwrap();
    let plan = Plan::paths(
        Operation::Rename,
        vec![source.clone()],
        Some(target.clone()),
    )
    .unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert_eq!(record.entries[0].outcome, Outcome::Completed);
    assert_eq!(
        fs::read_link(&target).unwrap(),
        std::path::Path::new("sentinel")
    );
    assert_eq!(
        terminal_file_manager::fs::inspect(&target)
            .identity
            .unwrap()
            .object,
        plan.operands[0].identity.as_ref().unwrap().object
    );
    assert!(fs::symlink_metadata(source).is_err());
    assert_eq!(
        fs::read(root.path().join("sentinel")).unwrap(),
        b"unchanged"
    );
}
