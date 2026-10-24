use std::{fs, process::Command};
use terminal_file_manager::operations::{Decision, Operation, Options, Plan, execute};

#[test]
fn native_trash_preserves_selected_tree_and_reports_actual_recursive_counts() {
    #[cfg(any(target_os = "macos", windows))]
    if std::env::var("RUNNER_ENVIRONMENT").as_deref() != Ok("github-hosted") {
        eprintln!(
            "NOT RUN: native Trash requires an ephemeral GitHub-hosted identity; developer Trash is excluded"
        );
        return;
    }
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("selected/nested")).unwrap();
    fs::create_dir(root.path().join("data")).unwrap();
    fs::write(
        root.path().join("selected/nested/file"),
        "owned native fixture",
    )
    .unwrap();
    // Relaunches this test with an isolated fixture, without a security decision.
    // nosemgrep: rust.lang.security.current-exe.current-exe
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "trash_child", "--nocapture"])
        .env("TFM_NATIVE_TRASH_ROOT", root.path())
        .env("HOME", root.path())
        .env("XDG_DATA_HOME", root.path().join("data"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("NATIVE_TRASH_EXERCISED"));
    assert!(!root.path().join("selected").exists());
}

#[test]
#[ignore = "owned child entry point; invoked by the native Trash isolation parent"]
fn trash_child() {
    let root = std::path::PathBuf::from(
        std::env::var_os("TFM_NATIVE_TRASH_ROOT").expect("owned fixture required"),
    );
    let plan = Plan::paths(Operation::Trash, vec![root.join("selected")], None).unwrap();
    #[cfg(not(windows))]
    let selected_identity = plan.operands[0].identity.as_ref().unwrap().object;
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert_eq!(record.completed_files(), 0, "{:?}", record.entries);
    assert_eq!(record.completed_directories(), 1, "{:?}", record.entries);
    assert_eq!(record.entries.len(), 1, "one native top-level operation");
    assert!(!root.join("selected").exists());
    let evidence = record.entries[0].trash.as_ref().unwrap();
    assert_eq!(
        (evidence.validated_files, evidence.validated_directories),
        (1, 2)
    );
    #[cfg(not(windows))]
    verify_recovery(&record.entries[0], selected_identity);
    #[cfg(windows)]
    {
        assert!(record.entries[0].destination.is_none());
        assert!(matches!(
            &evidence.receipt,
            terminal_file_manager::operations::trash::Receipt::Windows {
                parsing_name: Some(_),
                ..
            }
        ));
    }
    println!("NATIVE_TRASH_EXERCISED");
    #[cfg(target_os = "linux")]
    trash_metadata_claim(&root);
    #[cfg(target_os = "linux")]
    late_scope_changes(&root);
    #[cfg(target_os = "linux")]
    independent_leaf_scopes(&root);
}

#[cfg(target_os = "linux")]
fn independent_leaf_scopes(root: &std::path::Path) {
    use terminal_file_manager::operations::Outcome;
    let paths: Vec<_> = (0..5)
        .map(|index| root.join(format!("approved-{index}")))
        .collect();
    for path in &paths {
        fs::write(path, "approved bytes").unwrap();
    }
    let plan = counted(Plan::paths(Operation::Trash, paths.clone(), None).unwrap());
    let retained = terminal_file_manager::fs::inspect(&paths[2])
        .identity
        .unwrap()
        .object;
    fs::write(&paths[2], "changed bytes retained").unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert_eq!(record.completed_files(), 4, "{record:?}");
    assert_eq!(record.entries.len(), 5);
    for entry in &record.entries {
        if entry.source == paths[2] {
            assert_eq!(entry.outcome, Outcome::Changed);
            assert!(entry.trash.is_none());
        } else {
            assert_eq!(entry.outcome, Outcome::Completed);
            assert!(!entry.source.exists());
            let receipt = entry.trash.as_ref().unwrap();
            assert_eq!(
                (receipt.validated_files, receipt.validated_directories),
                (1, 0)
            );
            assert_eq!(
                fs::read(entry.destination.as_ref().unwrap()).unwrap(),
                b"approved bytes"
            );
        }
    }
    assert_eq!(fs::read(&paths[2]).unwrap(), b"changed bytes retained");
    assert_eq!(
        terminal_file_manager::fs::inspect(&paths[2])
            .identity
            .unwrap()
            .object,
        retained
    );
    println!("INDEPENDENT_APPROVED_TRASH_ROOTS_EXERCISED");
}

#[cfg(target_os = "linux")]
fn late_scope_changes(root: &std::path::Path) {
    fs::create_dir(root.join("outside")).unwrap();
    fs::write(root.join("outside/sentinel"), "outside preserved").unwrap();
    for change in ["write", "replace", "new", "link", "cancel"] {
        late_scope_case(root, change);
    }
    assert_eq!(
        fs::read(root.join("outside/sentinel")).unwrap(),
        b"outside preserved"
    );
    println!("LATE_TRASH_SCOPE_REFUSALS_EXERCISED");
}

#[cfg(target_os = "linux")]
fn late_scope_case(root: &std::path::Path, change: &str) {
    use terminal_file_manager::operations::{Event, Outcome, Stage};
    let selected = root.join(format!("late-{change}"));
    fs::create_dir_all(selected.join("nested")).unwrap();
    fs::write(selected.join("nested/file"), "original").unwrap();
    let plan = counted(Plan::paths(Operation::Trash, vec![selected.clone()], None).unwrap());
    let file_object = terminal_file_manager::fs::inspect(&selected.join("nested/file"))
        .identity
        .unwrap()
        .object;
    let before = terminal_file_manager::fs::inspect(&selected)
        .identity
        .unwrap()
        .object;
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut observed = false;
    let record = execute(&plan, Options::default(), &cancel, |event| {
        if matches!(
            event,
            Event::Progress {
                stage: Stage::Removing,
                ..
            }
        ) {
            alter_scope(root, &selected, change, &cancel);
            observed = true;
        }
        Decision::Skip
    });
    let expected = if change == "cancel" {
        Outcome::Cancelled
    } else {
        Outcome::Changed
    };
    assert_eq!(record.entries[0].outcome, expected, "{change}: {record:?}");
    assert!(observed, "exercise the actual pre-submission boundary");
    assert_eq!(
        terminal_file_manager::fs::inspect(&selected)
            .identity
            .unwrap()
            .object,
        before
    );
    assert!(record.entries[0].trash.is_none());
    retained_scope(root, &selected, change, file_object);
}

#[cfg(target_os = "linux")]
fn retained_scope(
    root: &std::path::Path,
    selected: &std::path::Path,
    change: &str,
    original: Option<(u64, u64)>,
) {
    assert_eq!(
        fs::read_dir(root.join("data/Trash/files")).unwrap().count(),
        1
    );
    assert_eq!(
        fs::read_dir(root.join("data/Trash/info")).unwrap().count(),
        1
    );
    let retained = if change == "link" {
        root.join("retained-directory/file")
    } else {
        selected.join("nested/file")
    };
    let expected_bytes = if matches!(change, "write" | "replace") {
        b"external change".as_slice()
    } else {
        b"original".as_slice()
    };
    assert_eq!(fs::read(&retained).unwrap(), expected_bytes);
    let current = terminal_file_manager::fs::inspect(&retained)
        .identity
        .unwrap()
        .object;
    if change == "replace" {
        assert_eq!(fs::read(root.join("retained-file")).unwrap(), b"original");
        assert_eq!(
            terminal_file_manager::fs::inspect(&root.join("retained-file"))
                .identity
                .unwrap()
                .object,
            original
        );
        assert_ne!(current, original);
    } else {
        assert_eq!(current, original);
    }
    if change == "new" {
        assert_eq!(
            fs::read(selected.join("nested/new")).unwrap(),
            b"unapproved"
        );
    }
}

#[cfg(target_os = "linux")]
fn counted(plan: Plan) -> Plan {
    use std::time::Duration;
    use terminal_file_manager::operations::{Queue, Update};
    let mut queue = Queue::new().unwrap();
    queue.preview(plan).unwrap();
    loop {
        match queue.next(Duration::from_secs(10)).unwrap() {
            Update::Preview { plan, .. } => return *plan,
            Update::Finished { record, .. } => panic!("preview failed: {record:?}"),
            _ => {}
        }
    }
}

#[cfg(target_os = "linux")]
fn alter_scope(
    root: &std::path::Path,
    selected: &std::path::Path,
    change: &str,
    cancel: &std::sync::atomic::AtomicBool,
) {
    match change {
        "write" => fs::write(selected.join("nested/file"), "external change").unwrap(),
        "replace" => {
            fs::rename(selected.join("nested/file"), root.join("retained-file")).unwrap();
            fs::write(selected.join("nested/file"), "external change").unwrap();
        }
        "new" => fs::write(selected.join("nested/new"), "unapproved").unwrap(),
        "link" => {
            fs::rename(selected.join("nested"), root.join("retained-directory")).unwrap();
            std::os::unix::fs::symlink(root.join("outside"), selected.join("nested")).unwrap();
        }
        "cancel" => cancel.store(true, std::sync::atomic::Ordering::Relaxed),
        _ => panic!("unknown scope change"),
    }
}

#[cfg(not(windows))]
fn verify_recovery(
    entry: &terminal_file_manager::operations::EntryResult,
    selected_identity: Option<(u64, u64)>,
) {
    if let Some(receipt) = entry.destination.as_ref() {
        assert_eq!(
            terminal_file_manager::fs::inspect(receipt)
                .identity
                .unwrap()
                .object,
            selected_identity
        );
        assert_eq!(
            fs::read(receipt.join("nested/file")).unwrap(),
            b"owned native fixture"
        );
    } else {
        assert!(matches!(
            entry.trash.as_ref().unwrap().receipt,
            terminal_file_manager::operations::trash::Receipt::Mac { path: None }
        ));
    }
}

#[cfg(target_os = "linux")]
fn trash_metadata_claim(root: &std::path::Path) {
    let source = root.join("queued-source");
    fs::write(&source, "preserved").unwrap();
    let metadata = fs::read_dir(root.join("data/Trash/info"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let trash = Plan::paths(Operation::Trash, vec![source], None).unwrap();
    let deletion = Plan::paths(Operation::Delete, vec![metadata.clone()], None).unwrap();
    let mut coordinator = terminal_file_manager::operations::Coordinator::default();
    assert!(coordinator.acquire(1, &trash).unwrap());
    assert!(!coordinator.acquire(2, &deletion).unwrap());
    assert!(metadata.is_file());
}
