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
fn native_case_only_rename_preserves_identity_and_requested_spelling() {
    for directory in [false, true] {
        for (old, new) in [("Case-雪", "case-雪"), ("CASE-Γ", "case-γ")] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join(old);
            let target = root.path().join(new);
            make(&source, directory, b"selected bytes");
            let insensitive = target.exists();
            #[cfg(any(windows, target_os = "macos"))]
            assert!(
                insensitive,
                "Native case-insensitive assurance needs a case-insensitive test volume"
            );
            let plan = Plan::paths(Operation::Rename, vec![source], Some(target.clone())).unwrap();
            let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
            assert_eq!(record.entries[0].outcome, Outcome::Completed, "{record:?}");
            assert_eq!(bytes(&target, directory), b"selected bytes");
            assert_eq!(
                terminal_file_manager::fs::inspect(&target).identity,
                plan.operands[0].identity
            );
            let names: Vec<_> = fs::read_dir(root.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            assert_eq!(names, vec![std::ffi::OsString::from(new)]);
            eprintln!(
                "NATIVE_CASE_RENAME_EXERCISED: insensitive={insensitive}, directory={directory}, {old} -> {new}"
            );
        }
    }
}

#[test]
fn a_distinct_hard_link_name_is_not_a_case_only_rename() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("selected");
    let target = root.path().join("alias");
    fs::write(&source, "retained").unwrap();
    fs::hard_link(&source, &target).unwrap();
    let plan = Plan::paths(
        Operation::Rename,
        vec![source.clone()],
        Some(target.clone()),
    )
    .unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert_ne!(record.entries[0].outcome, Outcome::Completed);
    assert_eq!(fs::read(&source).unwrap(), b"retained");
    assert_eq!(fs::read(&target).unwrap(), b"retained");
    assert_eq!(
        terminal_file_manager::fs::inspect(&source).identity,
        plan.operands[0].identity
    );
    assert_eq!(
        terminal_file_manager::fs::inspect(&target).identity,
        plan.operands[0].identity
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
}

#[test]
fn case_only_rename_refuses_a_source_replaced_at_the_native_boundary() {
    for directory in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("Case-雪");
        let target = root.path().join("case-雪");
        let retained = root.path().join("retained");
        make(&source, directory, b"selected bytes");
        let plan = Plan::paths(Operation::Rename, vec![source.clone()], Some(target)).unwrap();
        let mut replaced = false;
        let record = execute(&plan, Options::default(), &false.into(), |event| {
            if matches!(
                event,
                Event::Progress {
                    stage: Stage::Verified,
                    ..
                }
            ) {
                fs::rename(&source, &retained).unwrap();
                make(&source, directory, b"external replacement");
                replaced = true;
            }
            Decision::Skip
        });
        assert!(replaced);
        assert_eq!(record.entries[0].outcome, Outcome::Changed);
        assert_eq!(bytes(&retained, directory), b"selected bytes");
        assert_eq!(bytes(&source, directory), b"external replacement");
        assert_eq!(
            terminal_file_manager::fs::inspect(&retained).identity,
            plan.operands[0].identity
        );
        assert_ne!(
            terminal_file_manager::fs::inspect(&source).identity,
            plan.operands[0].identity
        );
        let names: Vec<_> = fs::read_dir(root.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert!(names.contains(&std::ffi::OsString::from("Case-雪")));
        assert!(!names.contains(&std::ffi::OsString::from("case-雪")));
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
