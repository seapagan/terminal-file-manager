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
