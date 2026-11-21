//! Owned native fixtures; manual Shell decisions never run in the default suite.
use super::*;
use crate::operations::{self, Decision, Operation, Options, Plan, Record};
use std::io::{Read, Write};

fn object(path: &Path) -> (u64, u64) {
    crate::fs::inspect(path)
        .identity
        .unwrap()
        .object
        .expect("native object identity required")
}

fn execute(plan: &Plan, cancel: bool) -> Record {
    operations::execute(plan, Options::default(), &cancel.into(), |_| Decision::Skip)
}

fn plan(path: &Path) -> Plan {
    Plan::paths(Operation::Trash, vec![path.to_owned()], None).unwrap()
}

#[test]
fn native_cancelled_trash_pipeline_retains_the_selected_object() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("selected");
    fs::write(&source, b"retained").unwrap();
    let before = object(&source);
    let record = execute(&plan(&source), true);
    assert_eq!(record.entries.len(), 1);
    assert_eq!(record.entries[0].outcome, Outcome::Cancelled);
    assert!(record.entries[0].trash.is_none());
    assert_eq!(object(&source), before);
    assert_eq!(fs::read(source).unwrap(), b"retained");
}

#[test]
fn native_trash_pipeline_never_adopts_a_replaced_selected_object() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("selected");
    let retained = root.path().join("retained-original");
    fs::write(&source, b"original").unwrap();
    let original = object(&source);
    let plan = plan(&source);
    fs::rename(&source, &retained).unwrap();
    fs::write(&source, b"replacement").unwrap();
    let replacement = object(&source);
    let record = execute(&plan, false);
    assert_eq!(record.entries[0].outcome, Outcome::Changed);
    assert!(record.entries[0].trash.is_none());
    assert_eq!(object(&source), replacement);
    assert_eq!(object(&retained), original);
    assert_eq!(fs::read(source).unwrap(), b"replacement");
    assert_eq!(fs::read(retained).unwrap(), b"original");
}

#[test]
#[ignore = "owned Recycle Bin fixture; announce the test and obtain maintainer readiness first"]
fn manual_windows_trash_ordinary_restores_the_exact_native_receipt() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("tfm-ordinary-recycle.txt");
    fs::write(&source, b"recoverable fixture").unwrap();
    let before = object(&source);
    let record = execute(&plan(&source), false);
    assert_eq!(record.entries.len(), 1);
    assert_eq!(record.entries[0].outcome, Outcome::Completed);
    assert_eq!(record.completed_files(), 1);
    assert!(!source.exists());
    let evidence = record.entries[0].trash.as_ref().unwrap();
    assert!(matches!(
        &evidence.receipt,
        Receipt::Windows {
            parsing_name: Some(_),
            ..
        }
    ));
    assert!(record.entries[0].destination.is_none());
    restore(&evidence.receipt, &source);
    assert_eq!(object(&source), before);
    assert_eq!(fs::read(source).unwrap(), b"recoverable fixture");
    eprintln!("NATIVE_TRASH_RECEIPT_RESTORED: {before:?}");
}

fn oversized_fixture(name: &str) -> (tempfile::TempDir, PathBuf, (u64, u64)) {
    use std::os::windows::fs::MetadataExt;
    let bytes: u64 = std::env::var("TFM_WINDOWS_TRASH_TEST_BYTES")
        .expect("set an oversized logical length above the existing volume bin limit")
        .parse()
        .unwrap();
    assert!((1..=64 * 1024 * 1024 * 1024).contains(&bytes));
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join(name);
    fs::write(&source, b"owned sparse recovery fixture").unwrap();
    let sparse = std::process::Command::new("fsutil")
        .args(["sparse", "setflag"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(sparse.status.success(), "{sparse:?}");
    let mut file = fs::OpenOptions::new().write(true).open(&source).unwrap();
    assert_ne!(file.metadata().unwrap().file_attributes() & 0x200, 0);
    file.set_len(bytes).unwrap();
    file.flush().unwrap();
    drop(file);
    fs::hard_link(&source, root.path().join("recovery-anchor")).unwrap();
    let before = object(&source);
    assert_eq!(object(&root.path().join("recovery-anchor")), before);
    (root, source, before)
}

fn assert_payload(path: &Path) {
    let mut payload = [0; 29];
    fs::File::open(path)
        .unwrap()
        .read_exact(&mut payload)
        .unwrap();
    assert_eq!(&payload, b"owned sparse recovery fixture");
}

#[test]
#[ignore = "manual Windows permanent-delete warning: announce first, then choose No"]
fn manual_windows_trash_permanent_refusal_retains_identity_and_payload() {
    let (root, source, before) = oversized_fixture("tfm-choose-NO.bin");
    eprintln!("Choose NO in the Windows warning for {}", source.display());
    let record = execute(&plan(&source), false);
    assert_eq!(record.entries.len(), 1);
    assert_eq!(record.entries[0].outcome, Outcome::Cancelled, "{record:?}");
    assert!(record.entries[0].trash.is_none());
    assert_eq!(record.completed_files(), 0);
    assert_eq!(object(&source), before);
    assert_eq!(object(&root.path().join("recovery-anchor")), before);
    assert_payload(&source);
    assert_payload(&root.path().join("recovery-anchor"));
    eprintln!("NATIVE_TRASH_NO_RETAINED: {before:?}");
}

#[test]
#[ignore = "manual Windows permanent-delete warning: announce first, then choose Yes; owned anchor preserves payload"]
fn manual_windows_trash_permanent_approval_reports_irreversible_success() {
    let (root, source, before) = oversized_fixture("tfm-choose-YES.bin");
    eprintln!("Choose YES in the Windows warning for {}", source.display());
    let record = execute(&plan(&source), false);
    assert_eq!(record.entries.len(), 1);
    assert_eq!(
        record.entries[0].outcome,
        Outcome::PermanentlyDeleted,
        "{record:?}"
    );
    assert!(record.entries[0].trash.is_none());
    assert_eq!(record.completed_files(), 0);
    assert_eq!(
        fs::symlink_metadata(&source).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    assert_eq!(object(&root.path().join("recovery-anchor")), before);
    assert_payload(&root.path().join("recovery-anchor"));
    eprintln!("NATIVE_TRASH_YES_IRREVERSIBLE: preserved anchor {before:?}");
}
