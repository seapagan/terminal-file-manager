use super::*;
use std::{fs, path::PathBuf};

fn fixture() -> Option<tempfile::TempDir> {
    if std::env::var("RUNNER_ENVIRONMENT").as_deref() != Ok("github-hosted") {
        eprintln!("NOT RUN: real Windows Trash requires an ephemeral hosted identity");
        return None;
    }
    Some(tempfile::tempdir().unwrap())
}

#[test]
fn native_shell_restores_the_owned_recursive_tree() {
    let Some(root) = fixture() else {
        return;
    };
    let source = root.path().join("selected");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::write(source.join("nested/file"), "native recoverable fixture").unwrap();
    let receipt = put(&crate::fs::inspect(&source), &false.into()).unwrap();
    assert!(!source.exists());
    assert!(receipt.path().is_none());
    restore(&receipt, &source);
    assert_eq!(
        fs::read(source.join("nested/file")).unwrap(),
        b"native recoverable fixture"
    );
}

#[test]
fn native_shell_recycles_the_link_object_without_touching_its_target() {
    let Some(root) = fixture() else {
        return;
    };
    let target = root.path().join("target");
    let source = root.path().join("selected-link");
    fs::write(&target, "sentinel").unwrap();
    std::os::windows::fs::symlink_file(&target, &source).unwrap();
    let entry = crate::fs::inspect(&source);
    let receipt = put(&entry, &false.into()).unwrap();
    assert!(fs::symlink_metadata(&source).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"sentinel");
    restore(&receipt, &source);
    assert_eq!(fs::read_link(&source).unwrap(), entry.link_target.unwrap());
    assert_eq!(fs::read(target).unwrap(), b"sentinel");
}

#[test]
fn native_shell_restores_an_ordinary_file_and_preserves_a_junction_target() {
    let Some(root) = fixture() else {
        return;
    };
    let file = root.path().join("ordinary-雪");
    fs::write(&file, "file fixture").unwrap();
    let receipt = put(&crate::fs::inspect(&file), &false.into()).unwrap();
    assert!(!file.exists());
    restore(&receipt, &file);
    assert_eq!(fs::read(&file).unwrap(), b"file fixture");
    let target = root.path().join("target");
    let junction = root.path().join("junction");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("sentinel"), "preserved").unwrap();
    let created = std::process::Command::new("cmd")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .output()
        .unwrap();
    assert!(created.status.success());
    let entry = crate::fs::inspect(&junction);
    assert_eq!(entry.kind, EntryKind::Junction);
    let receipt = put(&entry, &false.into()).unwrap();
    assert!(fs::symlink_metadata(&junction).is_err());
    restore(&receipt, &junction);
    assert_eq!(crate::fs::inspect(&junction).kind, EntryKind::Junction);
    assert_eq!(fs::read(target.join("sentinel")).unwrap(), b"preserved");
    fs::remove_dir(junction).unwrap();
}

#[test]
fn stale_source_and_pre_submission_cancellation_never_enter_the_shell_queue() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("file");
    fs::write(&source, "retained").unwrap();
    let entry = crate::fs::inspect(&source);
    assert_eq!(
        put(&entry, &true.into()).unwrap_err().outcome,
        Outcome::Cancelled
    );
    fs::rename(&source, root.path().join("retained-original")).unwrap();
    fs::write(&source, "replacement").unwrap();
    assert_eq!(
        put(&entry, &false.into()).unwrap_err().outcome,
        Outcome::Changed
    );
    assert_eq!(fs::read(source).unwrap(), b"replacement");
}

fn restore(receipt: &Receipt, destination: &Path) {
    let Receipt::Windows {
        parsing_name: Some(name),
        ..
    } = receipt
    else {
        panic!("Native test requires a supported Shell recovery reference");
    };
    let source = PathBuf::from(std::ffi::OsString::from_wide(name));
    std::thread::scope(|scope| {
        scope
            .spawn(|| restore_sta(&source, destination))
            .join()
            .unwrap()
    })
    .unwrap();
}

#[allow(
    unsafe_code,
    reason = "Approved native Shell boundary: restore only this test's opaque receipt to its owned fixture"
)]
fn restore_sta(source: &Path, destination: &Path) -> io::Result<()> {
    let _apartment = Apartment::initialize()?;
    let recycled = shell_item(source)?;
    let parent = shell_item(destination.parent().unwrap())?;
    let name = wide(Path::new(destination.file_name().unwrap()))?;
    assert!(fs::symlink_metadata(destination).is_err());
    // SAFETY: interfaces and UTF-16 buffer remain live in this dedicated STA;
    // only the positively identified owned test receipt is moved to its empty name.
    unsafe {
        let operation =
            configured(FOF_NO_CONNECTED_ELEMENTS | FOF_SILENT | FOF_NOERRORUI | FOFX_EARLYFAILURE)?;
        operation
            .MoveItem(
                &recycled,
                &parent,
                PCWSTR(name.as_ptr()),
                None::<&IFileOperationProgressSink>,
            )
            .map_err(native_error)?;
        let performed = operation.PerformOperations();
        let aborted = operation.GetAnyOperationsAborted();
        performed.map_err(native_error)?;
        if aborted.map_err(native_error)?.as_bool() {
            return Err(io::Error::other("Owned Shell restore aborted"));
        }
    }
    Ok(())
}

use std::os::windows::ffi::OsStringExt;
