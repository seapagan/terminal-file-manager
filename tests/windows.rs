//! Native Windows metadata/reparse contracts; Linux xwin supplies compile evidence only.
#![cfg(windows)]
use std::{fs, process::Command, sync::atomic::AtomicBool};
use terminal_file_manager::{
    Invocation,
    fs::{self as local, EntryKind, IgnoreContext},
};

#[test]
fn windows_hidden_attribute_is_independent_of_dot_names() {
    let root = tempfile::tempdir().unwrap();
    let dotted = root.path().join(".dot-name");
    fs::write(&dotted, "data").unwrap();
    assert!(!local::inspect(&dotted).hidden);
    let hidden = root.path().join("hidden-name");
    fs::write(&hidden, "data").unwrap();
    let output = Command::new("attrib")
        .arg("+h")
        .arg(&hidden)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let entry = local::inspect(&hidden);
    assert!(entry.hidden);
    assert_eq!(entry.kind, EntryKind::File);
    assert_eq!(fs::read(&hidden).unwrap(), b"data");
}

#[test]
fn junctions_are_native_nonrecursive_and_preserve_trailing_paths() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    let junction = root.path().join("junction");
    fs::create_dir(&target).unwrap();
    let output = Command::new("cmd")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let snapshot = local::list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let link = snapshot
        .entries
        .iter()
        .find(|e| e.path == junction)
        .unwrap();
    assert_eq!(link.kind, EntryKind::Junction);
    assert!(link.enterable);
    assert!(!link.may_recurse());
    assert!(link.link_target.is_some());
    let mut trailing = junction.as_os_str().to_owned();
    trailing.push("\\");
    let trailing_entry = local::inspect(std::path::Path::new(&trailing));
    assert_eq!(trailing_entry.kind, EntryKind::Junction);
    assert_eq!(
        trailing_entry.identity.unwrap().object,
        link.identity.as_ref().unwrap().object
    );
    assert!(local::totals(&junction, &AtomicBool::new(false)).is_err());
    fs::remove_dir(&target).unwrap();
    let broken = local::inspect(&junction);
    assert_eq!(broken.kind, EntryKind::Junction);
    assert!(broken.error.is_some());
    fs::remove_dir(&junction).unwrap();
}

#[test]
fn unc_drive_and_extended_paths_survive_cli_parsing_without_network_access() {
    for path in [
        r"\\server\share\directory",
        r"C:\directory with spaces",
        r"\\?\C:\long\directory",
    ] {
        let result = terminal_file_manager::run(&[path.into()]).unwrap();
        match result {
            Invocation::Browse(native) => assert_eq!(native, std::path::PathBuf::from(path)),
            _ => panic!("path was not preserved"),
        }
    }
}
