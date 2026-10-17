//! Count completions cannot revive stale selection or cancelled work.
use std::{fs, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::App,
    fs::{IgnoreContext, Totals, list},
};

#[test]
fn counts_reject_cursor_changes_cancellation_and_old_generations() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("a")).unwrap();
    fs::create_dir(root.path().join("b")).unwrap();
    let mut app = App::new(root.path().to_path_buf());
    app.complete_listing(
        0,
        1,
        Ok(list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    app.request_metadata(0).unwrap();
    let first = app.request_totals().unwrap();
    app.panes[0].move_cursor(1, 10);
    assert!(!app.complete_totals(&first, Ok(Totals::default())));
    app.request_metadata(0).unwrap();
    let second = app.request_totals().unwrap();
    app.cancel_totals();
    assert!(!app.complete_totals(&second, Ok(Totals::default())));
    let third = app.request_totals().unwrap();
    assert!(!app.complete_totals(&second, Ok(Totals::default())));
    assert!(app.complete_totals(&third, Ok(Totals::default())));
}

#[test]
#[cfg(unix)]
fn selected_link_is_not_a_recursive_count_root() {
    let root = tempfile::tempdir().unwrap();
    let link = root.path().join("link");
    std::os::unix::fs::symlink(root.path(), &link).unwrap();
    assert!(terminal_file_manager::fs::totals(&link, &AtomicBool::new(false)).is_err());
}

#[test]
fn same_path_replacement_refresh_reloads_metadata_and_rejects_old_responses() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("selected");
    fs::write(&path, "old").unwrap();
    let snapshot = || {
        list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()
    };
    let mut app = App::new(root.path().to_path_buf());
    app.complete_listing(0, 1, Ok(snapshot()));
    let old_request = app.request_metadata(0).unwrap();
    let old = terminal_file_manager::fs::inspect(&path);
    assert!(app.complete_metadata(&old_request, old.clone(), Some(path.clone())));
    assert!(app.request_metadata(0).is_none());
    // Keep the old object alive so native IDs cannot be recycled for its replacement.
    fs::rename(&path, root.path().join("retained-old")).unwrap();
    fs::write(&path, "replacement").unwrap();
    let request = app.request_listing(0, root.path().to_path_buf());
    assert!(app.complete_listing(0, request.generation, Ok(snapshot())));
    assert_eq!(app.panes[0].current().unwrap().path, path);
    let fresh_request = app.request_metadata(0).unwrap();
    assert!(fresh_request.generation > old_request.generation);
    assert!(app.details[0].entry.is_none());
    assert!(app.details[0].canonical.is_none());
    let fresh = terminal_file_manager::fs::inspect(&path);
    assert_ne!(
        old.identity.as_ref().unwrap().object,
        fresh.identity.as_ref().unwrap().object
    );
    assert!(!app.complete_metadata(&old_request, old.clone(), None));
    assert!(app.complete_metadata(&fresh_request, fresh.clone(), Some(path.clone())));
    assert!(!app.complete_metadata(&old_request, old, None));
    assert_eq!(
        app.details[0].entry.as_ref().unwrap().identity,
        fresh.identity
    );
    assert_eq!(app.details[0].entry.as_ref().unwrap().size, Some(11));
}
