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
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert_eq!(record.completed_files(), 1, "{:?}", record.entries);
    assert_eq!(record.completed_directories(), 2, "{:?}", record.entries);
    let receipt = record.entries.last().unwrap().destination.as_ref().unwrap();
    assert_eq!(
        fs::read(receipt.join("nested/file")).unwrap(),
        b"owned native fixture"
    );
    println!("NATIVE_TRASH_EXERCISED");
}
