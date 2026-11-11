use super::*;

fn fixture() -> (tempfile::TempDir, Directory) {
    let root = tempfile::tempdir().unwrap();
    let mut app = crate::app::App::new(root.path().into());
    app.complete_listing(
        0,
        1,
        Ok(crate::fs::list(root.path(), &Default::default(), &Default::default()).unwrap()),
    );
    let directory = Directory::capture(&app.panes[0]).unwrap();
    (root, directory)
}

#[test]
fn directory_validation_rejects_independent_lexical_and_physical_failures() {
    for failure in 0..5 {
        let (_root, directory) = fixture();
        let mut calls = 0;
        let error = directory
            .validate(&mut |path| {
                calls += 1;
                let mut entry = crate::fs::inspect_identity(path);
                match (failure, calls) {
                    (0, 1) | (1, 2) => entry.error = Some("inspection failed".into()),
                    (2, 1) => entry.identity = None,
                    (3, 2) => entry.identity.as_mut().unwrap().object = Some((u64::MAX, u64::MAX)),
                    (4, 2) => entry.kind = crate::fs::EntryKind::File,
                    _ => {}
                }
                entry
            })
            .unwrap_err();
        assert_eq!(
            error,
            "Current directory was removed or replaced; refresh and start again"
        );
    }
}

#[test]
fn matching_inspection_evidence_cannot_hide_a_missing_native_directory() {
    let (root, directory) = fixture();
    let lexical = crate::fs::inspect_identity(&directory.path);
    let physical = crate::fs::inspect_identity(&directory.physical.path);
    std::fs::remove_dir(root.path()).unwrap();
    let mut calls = 0;
    assert!(
        directory
            .validate(&mut |_| {
                calls += 1;
                if calls == 1 {
                    lexical.clone()
                } else {
                    physical.clone()
                }
            })
            .is_err()
    );
}
