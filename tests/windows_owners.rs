#![cfg(windows)]
//! Native ownership stays independent of enumeration and cached presentation.
use std::sync::atomic::AtomicBool;
use terminal_file_manager::{fs, owner::OwnerDisplay};

#[test]
fn native_owner_sid_and_known_account_preserve_numeric_identity() {
    use terminal_file_manager::owner::windows::{Name, read_sid, resolve_name};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("owner-界");
    std::fs::write(&path, b"x").unwrap();
    let entry = fs::inspect(&path);
    let sid = read_sid(&path, entry.identity.as_ref().unwrap()).unwrap();
    assert!(sid.starts_with("S-1-"), "{sid}");
    assert!(matches!(resolve_name("S-1-5-18"), Name::Resolved(name) if name.contains('\\')));
    assert!(matches!(resolve_name("not a SID"), Name::Failed(_)));
    assert!(matches!(resolve_name("S-1-5-999"), Name::Unmapped));
    std::fs::remove_file(&path).unwrap();
    assert!(read_sid(&path, entry.identity.as_ref().unwrap()).is_err());
}

#[test]
fn cached_sid_distinguishes_pending_unmapped_and_failed_name_resolution() {
    use terminal_file_manager::owner::windows::{Name, Ownership};
    let mut entry = fs::inspect(tempfile::tempdir().unwrap().path());
    for (name, expected) in [
        (Name::Pending, "S-1-5-18 (resolving)"),
        (Name::Unmapped, "S-1-5-18"),
        (Name::Failed("denied".into()), "S-1-5-18 (lookup failed)"),
        (
            Name::Resolved("NT AUTHORITY\\SYSTEM".into()),
            "NT AUTHORITY\\SYSTEM",
        ),
    ] {
        entry.windows_owner = Ownership::Sid {
            sid: "S-1-5-18".into(),
            name,
        };
        assert_eq!(entry.owner_display(OwnerDisplay::Names), expected);
        assert_eq!(entry.owner_display(OwnerDisplay::Numeric), "S-1-5-18");
    }
    entry.windows_owner = Ownership::Unavailable("denied".into());
    assert_eq!(entry.owner_display(OwnerDisplay::Names), "unavailable");
}

#[test]
fn no_follow_query_rejects_a_different_object_and_accepts_the_link_itself() {
    use terminal_file_manager::owner::windows::read_sid;
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("target");
    let link = root.path().join("link");
    std::fs::write(&file, b"x").unwrap();
    std::os::windows::fs::symlink_file(&file, &link).unwrap();
    let target = fs::inspect_identity(&file);
    let symlink = fs::inspect_identity(&link);
    assert!(read_sid(&link, target.identity.as_ref().unwrap()).is_err());
    assert!(
        read_sid(&link, symlink.identity.as_ref().unwrap())
            .unwrap()
            .starts_with("S-1-")
    );
}

#[test]
fn enumeration_marks_ownership_pending_without_claiming_a_lookup_failure() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), b"x").unwrap();
    let snapshot = fs::list(
        root.path(),
        &fs::IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    for entry in snapshot.entries {
        assert!(entry.owner.is_none());
        assert_eq!(entry.owner_display(OwnerDisplay::Names), "resolving");
        assert_eq!(entry.owner_display(OwnerDisplay::Numeric), "resolving");
    }
}

#[test]
fn missing_metadata_reports_unavailable_ownership_instead_of_pending_work() {
    let root = tempfile::tempdir().unwrap();
    let entry = fs::inspect(&root.path().join("missing"));
    assert_eq!(entry.owner_display(OwnerDisplay::Names), "unavailable");
    assert!(entry.windows_owner.error().is_some());
}
