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
