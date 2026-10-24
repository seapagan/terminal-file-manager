//! Ordinary refresh must preserve browsing context and reject directory substitution.
use std::{path::Path, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::App,
    fs::{self, IgnoreContext, Snapshot},
};

fn snapshot(path: &Path) -> Snapshot {
    fs::list(path, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap()
}

fn loaded(path: &Path) -> App {
    let mut app = App::new(path.into());
    app.complete_listing(0, 1, Ok(snapshot(path)));
    app
}

#[test]
fn deleted_cursor_keeps_nearest_row_and_deep_viewport() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..2000 {
        std::fs::write(root.path().join(format!("item{index:04}")), "data").unwrap();
    }
    let mut app = loaded(root.path());
    app.panes[0].move_cursor(1500, 20);
    let removed = app.panes[0].cursor.clone().unwrap();
    let scroll = app.panes[0].scroll;
    std::fs::remove_file(removed).unwrap();
    let request = app.request_refresh(0).unwrap();
    app.complete_listing(0, request.generation, Ok(snapshot(root.path())));
    assert_eq!(app.panes[0].current().unwrap().name, "item1501");
    assert_eq!(app.panes[0].scroll, scroll);
}

#[test]
fn missing_directory_evidence_retains_last_good_snapshot() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("original"), "data").unwrap();
    let mut app = loaded(root.path());
    let mut untrusted = snapshot(root.path());
    untrusted.directory = None;
    untrusted.entries.clear();
    let request = app.request_refresh(0).unwrap();
    app.complete_listing(0, request.generation, Ok(untrusted));
    assert_eq!(app.panes[0].entries.len(), 1);
    assert!(app.panes[0].error.is_some());
}

#[test]
fn refresh_defers_once_without_stealing_navigation_or_late_changes() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut app = loaded(root.path());
    let first = app.request_refresh(0).unwrap();
    for _ in 0..5000 {
        assert!(app.request_refresh(0).is_none());
    }
    assert_eq!(app.panes[0].generation, first.generation);
    app.complete_listing(0, first.generation, Ok(snapshot(root.path())));
    let final_scan = app.take_refresh(0).unwrap();
    assert!(app.take_refresh(0).is_none());
    let navigation = app.request_listing(0, other.path().into());
    assert!(app.request_refresh(0).is_none());
    assert!(!app.complete_listing(0, final_scan.generation, Ok(snapshot(root.path()))));
    app.complete_listing(0, navigation.generation, Ok(snapshot(other.path())));
    assert_eq!(app.panes[0].path, other.path());
    assert!(app.take_refresh(0).is_none());
}

#[test]
fn ordinary_refresh_does_not_end_a_selection_gesture() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a", "b", "c", "d", "e"] {
        std::fs::write(root.path().join(name), "data").unwrap();
    }
    let mut app = loaded(root.path());
    for _ in 0..3 {
        app.panes[0].extend_range(1, 20);
    }
    let request = app.request_refresh(0).unwrap();
    app.complete_listing(0, request.generation, Ok(snapshot(root.path())));
    app.panes[0].extend_range(-1, 20);
    assert_eq!(app.panes[0].marks.len(), 3);
    assert!(!app.panes[0].marks.contains_key(&root.path().join("d")));
}

#[test]
fn replaced_directory_is_stale_until_explicit_reopen_and_blocks_mutations() {
    use terminal_file_manager::operations::{Operation, Plan};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("current");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("original"), "data").unwrap();
    let mut app = loaded(&path);
    app.panes[0].toggle_mark();
    std::fs::rename(&path, root.path().join("old")).unwrap();
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("replacement"), "new").unwrap();
    let request = app.request_refresh(0).unwrap();
    app.complete_listing(0, request.generation, Ok(snapshot(&path)));
    assert_eq!(app.panes[0].current().unwrap().name, "original");
    assert!(app.panes[0].error.is_some());
    assert!(Plan::capture(&app.panes[0], Operation::Delete, None).is_err());
    let mut target = Plan::mkdir(path.join("new-directory"));
    assert!(target.capture_destination(&app.panes[0]).is_err());
    let reopen = app.request_listing(0, path.clone());
    app.complete_listing(0, reopen.generation, Ok(snapshot(&path)));
    assert!(app.panes[0].error.is_none());
    assert_eq!(app.panes[0].current().unwrap().name, "replacement");
    assert!(app.panes[0].marks.is_empty());
}

#[test]
fn manual_refresh_cannot_replace_pending_navigation() {
    use terminal_file_manager::{
        input::{Action, Bindings, Controller},
        ui,
    };
    let root = tempfile::tempdir().unwrap();
    let next = tempfile::tempdir().unwrap();
    let mut app = loaded(root.path());
    let request = app.request_listing(0, next.path().into());
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 100, 30));
    assert!(controller.dispatch(Action::Refresh, &geometry).is_empty());
    assert_eq!(
        controller.app.panes[0].pending.as_deref(),
        Some(next.path())
    );
    assert!(
        controller
            .app
            .complete_listing(0, request.generation, Ok(snapshot(next.path())))
    );
    assert_eq!(controller.app.panes[0].path, next.path());
}

#[test]
fn explicit_reopen_drops_marks_even_if_replacement_contains_the_same_file_object() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("current");
    let previous = root.path().join("previous");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("item"), "data").unwrap();
    let mut app = loaded(&path);
    app.panes[0].toggle_mark();
    std::fs::rename(&path, &previous).unwrap();
    std::fs::create_dir(&path).unwrap();
    std::fs::hard_link(previous.join("item"), path.join("item")).unwrap();
    let reopen = app.request_listing(0, path.clone());
    app.complete_listing(0, reopen.generation, Ok(snapshot(&path)));
    assert!(app.panes[0].marks.is_empty());
}

#[test]
fn unverified_initial_context_cannot_automatically_adopt_a_new_identity() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("old-view"), "old").unwrap();
    let mut app = App::new(root.path().into());
    let mut unverified = snapshot(root.path());
    unverified.directory = None;
    app.complete_listing(0, 1, Ok(unverified));
    std::fs::write(root.path().join("new-view"), "new").unwrap();
    let refresh = app.request_refresh(0).unwrap();
    app.complete_listing(0, refresh.generation, Ok(snapshot(root.path())));
    assert!(app.panes[0].directory.is_none());
    assert_eq!(app.panes[0].entries.len(), 1);
    assert!(app.panes[0].error.is_some());
}

#[test]
fn same_cursor_refresh_preserves_metadata_scroll_but_new_selection_resets_it() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a", "b"] {
        std::fs::write(root.path().join(name), "data").unwrap();
    }
    let mut app = loaded(root.path());
    app.request_metadata(0).unwrap();
    app.metadata_scroll = 17;
    let refresh = app.request_refresh(0).unwrap();
    app.complete_listing(0, refresh.generation, Ok(snapshot(root.path())));
    app.request_metadata(0).unwrap();
    assert_eq!(app.metadata_scroll, 17);
    app.panes[0].move_cursor(1, 20);
    app.request_metadata(0).unwrap();
    assert_eq!(app.metadata_scroll, 0);
}

#[test]
fn lost_directory_identity_invalidates_late_metadata_and_totals() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("child")).unwrap();
    let mut app = loaded(root.path());
    let metadata = app.request_metadata(0).unwrap();
    let entry = app.panes[0].current().unwrap().clone();
    let totals = app.request_totals().unwrap();
    let mut unknown = snapshot(root.path());
    unknown.directory = None;
    let refresh = app.request_refresh(0).unwrap();
    app.complete_listing(0, refresh.generation, Ok(unknown));
    assert!(!app.complete_metadata(&metadata, entry, None));
    assert!(!app.complete_totals(&totals, Ok(Default::default())));
    assert!(app.request_metadata(0).is_none());
    assert!(app.request_totals().is_none());
    assert!(!app.details[0].counting);
}

#[test]
fn refilter_uses_nearest_visible_index_and_empty_results_clear_the_cursor() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a0", "a1", "a2", "b0", "b1", "b2"] {
        std::fs::write(root.path().join(name), "data").unwrap();
    }
    let mut app = loaded(root.path());
    let pane = &mut app.panes[0];
    pane.move_cursor(4, 2);
    pane.select_all();
    pane.set_filter("a".into());
    assert_eq!(pane.current().unwrap().name, "a2");
    assert_eq!(pane.scroll, 2);
    assert_eq!(pane.marks.len(), 6);
    pane.set_filter("missing".into());
    assert!(pane.cursor.is_none());
    assert_eq!(pane.scroll, 0);
    assert!(pane.eligible().is_empty());
}
