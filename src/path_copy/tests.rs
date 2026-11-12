use super::*;
use std::sync::atomic::AtomicBool;

fn marked_app(count: usize) -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("template"), "").unwrap();
    let mut app = App::new(root.path().into());
    let listing =
        crate::fs::list(root.path(), &Default::default(), &AtomicBool::new(false)).unwrap();
    app.complete_listing(0, 1, Ok(listing));
    let pane = &mut app.panes[0];
    let template = pane.entries[0].clone();
    pane.entries = (0..count)
        .map(|i| {
            let mut entry = template.clone();
            entry.path = root.path().join(format!("entry-{i}"));
            entry.identity.as_mut().unwrap().object = Some((1, i as u64));
            entry
        })
        .collect();
    pane.visible = (0..count).rev().collect();
    pane.select_all();
    (root, app)
}

#[test]
fn large_marked_snapshot_preserves_order_after_visible_reordering() {
    let (_root, mut app) = marked_app(40_000);
    let snapshot = Snapshot::capture(&app).unwrap();
    app.panes[0].visible.reverse();
    let entries = snapshot.validate_marks(&app).unwrap();
    assert_eq!(entries.len(), 40_000);
    assert_eq!(entries, snapshot.marked);
}

#[test]
fn marked_snapshot_rejects_hidden_unmarked_replaced_and_reclassified_entries() {
    for change in 0..4 {
        let (_root, mut app) = marked_app(3);
        let snapshot = Snapshot::capture(&app).unwrap();
        let pane = &mut app.panes[0];
        match change {
            0 => {
                pane.visible.pop();
            }
            1 => {
                pane.marks.remove(&pane.entries[0].path);
            }
            2 => {
                pane.entries[0].identity.as_mut().unwrap().object = Some((2, 0));
            }
            _ => {
                pane.entries[0].kind = crate::fs::EntryKind::Directory;
            }
        }
        assert!(snapshot.validate_marks(&app).is_err());
    }
}

#[test]
fn activation_uses_frozen_cached_directories_without_touching_the_filesystem() {
    let (root, mut app) = marked_app(1);
    app.dual = true;
    let listing =
        crate::fs::list(root.path(), &Default::default(), &AtomicBool::new(false)).unwrap();
    app.complete_listing(1, 1, Ok(listing));
    std::fs::rename(root.path().join("template"), root.path().join("moved")).unwrap();
    let parent = tempfile::tempdir().unwrap();
    std::fs::rename(root.path(), parent.path().join("moved-root")).unwrap();
    let snapshot = Snapshot::capture(&app).expect("activation only captures cached state");
    assert!(snapshot.bases.choices()[1].disabled.is_none());
}

#[test]
fn identity_inspection_avoids_owner_resolution_and_preserves_native_evidence() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    std::fs::write(&file, "content").unwrap();
    let expected = crate::fs::inspect(&file);
    let fresh = crate::fs::inspect_identity(&file);
    assert!(fresh.owner.is_none());
    assert!(same_entry(&expected, &fresh));
    assert!(fresh.error.is_none());
    std::fs::remove_file(&file).unwrap();
    let missing = crate::fs::inspect_identity(&file);
    assert!(missing.error.is_some());
    assert!(missing.identity.is_none());
}

#[test]
fn captured_context_requires_exact_equality_including_zero_to_one() {
    let (_root, mut app) = marked_app(1);
    app.panes[0].context = 0;
    let snapshot = Snapshot::capture(&app).unwrap();
    assert!(snapshot.matches_context(&app));
    app.panes[0].context = 1;
    assert!(!snapshot.matches_context(&app));
}

#[test]
fn batch_text_and_provider_preflight_share_the_exact_payload_boundary() {
    let (root, app) = marked_app(1);
    for extra in [0, 1] {
        let mut entry = app.panes[0].entries[0].clone();
        entry.path = root
            .path()
            .join("x".repeat(crate::clipboard::MAX_BYTES + extra));
        let request = CopyRequest::new(
            vec![entry.clone()],
            Directory::capture(&app.panes[0]).unwrap(),
            Source::Highlighted,
            Format::Name,
            None,
            "name".into(),
        );
        let result = request.prepare_using(|path| {
            if path == entry.path {
                entry.clone()
            } else {
                crate::fs::inspect_identity(path)
            }
        });
        if extra == 0 {
            let text = result.unwrap();
            assert_eq!(text.len(), crate::clipboard::MAX_BYTES);
        } else {
            assert!(result.unwrap_err().contains("16 MiB"));
        }
    }
}
