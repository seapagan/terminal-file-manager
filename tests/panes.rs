//! Pure pane transitions and visible selection scope.
use std::{fs, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::{App, IgnoreMode},
    fs::{IgnoreContext, list},
};

fn loaded() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    for name in ["alpha", "beta", ".hidden", "ignored"] {
        fs::write(root.path().join(name), name).unwrap();
    }
    let mut snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    for entry in &mut snapshot.entries {
        entry.ignored = Some(entry.name == "ignored");
    }
    let mut app = App::new(root.path().to_path_buf());
    for pane in 0..2 {
        app.complete_listing(pane, 1, Ok(snapshot.clone()));
    }
    (root, app)
}

#[test]
fn selection_clear_and_zero_matches_obey_visible_scope() {
    let (_root, mut app) = loaded();
    let pane = &mut app.panes[0];
    pane.show_hidden = true;
    pane.refilter();
    pane.select_all();
    assert_eq!(pane.marks.len(), 4);
    pane.show_hidden = false;
    pane.ignore = IgnoreMode::Hide;
    pane.set_filter("alp".into());
    assert_eq!(pane.eligible().len(), 1);
    assert_eq!(pane.selection_counts().excluded(), 3);
    pane.clear_visible();
    assert_eq!(pane.marks.len(), 3);
    pane.set_filter("no matches".into());
    assert!(pane.eligible().is_empty());
    pane.select_all();
    pane.clear_visible();
    assert_eq!(pane.marks.len(), 3);
    assert!(app.panes[1].marks.is_empty());
}

#[test]
fn active_single_pane_and_width_fallback_preserve_requested_dual() {
    let (_root, mut app) = loaded();
    assert!(!app.dual);
    app.dual = true;
    app.active = 1;
    app.panes[1].set_filter("bet".into());
    assert_eq!(app.visible_panes(120), vec![0, 1]);
    assert_eq!(app.visible_panes(35), vec![1]);
    assert!(app.dual);
    assert_eq!(app.visible_panes(120), vec![0, 1]);
    app.dual = false;
    assert_eq!(app.visible_panes(120), vec![1]);
    assert_eq!(app.panes[1].filter, "bet");
}

#[test]
fn late_listing_cannot_replace_new_navigation_or_clear_its_error() {
    let (root, mut app) = loaded();
    let old = app.request_listing(0, root.path().join("old"));
    let new = app.request_listing(0, root.path().join("new"));
    app.complete_listing(0, new.generation, Err("selected new path denied".into()));
    let snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(!app.complete_listing(0, old.generation, Ok(snapshot)));
    assert!(
        app.panes[0]
            .error
            .as_ref()
            .unwrap()
            .contains("selected new path denied")
    );
    assert_eq!(app.panes[0].path, root.path());
}

#[test]
fn refresh_removes_marks_for_missing_or_replaced_objects() {
    let (root, mut app) = loaded();
    app.panes[0].select_all();
    let original = root.path().join("alpha");
    fs::rename(&original, root.path().join("old-object")).unwrap();
    fs::write(&original, "replacement").unwrap();
    fs::remove_file(root.path().join("beta")).unwrap();
    let request = app.request_listing(0, root.path().to_path_buf());
    let snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    app.complete_listing(0, request.generation, Ok(snapshot));
    assert!(!app.panes[0].marks.contains_key(&original));
    assert!(!app.panes[0].marks.contains_key(&root.path().join("beta")));
    assert!(
        app.panes[0]
            .marks
            .contains_key(&root.path().join("ignored"))
    );
}

#[test]
fn back_forward_restore_cursor_scroll_and_failed_navigation_keeps_history() {
    let (root, mut app) = loaded();
    app.panes[0].cursor = Some(root.path().join("beta"));
    app.panes[0].scroll = 1;
    let child = root.path().join("child");
    fs::create_dir(&child).unwrap();
    let request = app.request_listing(0, child.clone());
    let snapshot = list(&child, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap();
    app.complete_listing(0, request.generation, Ok(snapshot));
    let back = app.history(false).unwrap();
    app.complete_listing(0, back.generation, Err("denied".into()));
    let back = app.history(false).unwrap();
    let snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    app.complete_listing(0, back.generation, Ok(snapshot));
    assert_eq!(app.panes[0].cursor, Some(root.path().join("beta")));
    assert_eq!(app.panes[0].scroll, 1);
    assert_eq!(app.history(true).unwrap().path, child);
}

#[test]
fn late_metadata_cannot_replace_new_selection_or_refreshed_listing() {
    let (root, mut app) = loaded();
    app.panes[0].cursor = Some(root.path().join("alpha"));
    let old = app.request_metadata(0).unwrap();
    app.panes[0].cursor = Some(root.path().join("beta"));
    let new = app.request_metadata(0).unwrap();
    let entry = terminal_file_manager::fs::inspect(&new.path);
    assert!(app.complete_metadata(&new, entry, None));
    let entry = terminal_file_manager::fs::inspect(&old.path);
    assert!(!app.complete_metadata(&old, entry, None));
    assert_eq!(app.details[0].entry.as_ref().unwrap().name, "beta");
    let refresh = app.request_listing(0, root.path().to_path_buf());
    let snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    app.complete_listing(0, refresh.generation, Ok(snapshot));
    let entry = terminal_file_manager::fs::inspect(&new.path);
    assert!(!app.complete_metadata(&new, entry, None));
}

#[test]
fn large_marked_refresh_preserves_only_matching_native_identities() {
    use terminal_file_manager::fs::Snapshot;
    let (root, mut app) = loaded();
    let template = app.panes[0].entries[0].clone();
    let entries = (0..10_000)
        .map(|index| {
            let mut entry = template.clone();
            entry.name = format!("entry{index:05}").into();
            entry.path = root.path().join(&entry.name);
            entry.hidden = false;
            entry.identity.as_mut().unwrap().object = Some((1, index));
            entry
        })
        .collect();
    let mut snapshot = Snapshot {
        path: root.path().to_path_buf(),
        entries,
        errors: vec![],
    };
    app.complete_listing(0, 1, Ok(snapshot.clone()));
    app.panes[0].select_all();
    snapshot.entries[1].identity.as_mut().unwrap().object = Some((2, 1));
    snapshot.entries[2].identity = None;
    snapshot.entries.remove(0);
    let request = app.request_listing(0, root.path().to_path_buf());
    app.complete_listing(0, request.generation, Ok(snapshot));
    assert_eq!(app.panes[0].marks.len(), 9_997);
    for name in ["entry00000", "entry00001", "entry00002"] {
        assert!(!app.panes[0].marks.contains_key(&root.path().join(name)));
    }
    assert_eq!(app.panes[0].selection_counts().visible, 9_997);
}
