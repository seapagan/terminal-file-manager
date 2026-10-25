//! Parent navigation uses native entries and the ordinary asynchronous completion path.
use std::{path::Path, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::{App, Request},
    fs::{IgnoreContext, Snapshot, list},
    input::{Action, Bindings, Controller},
    ui,
};

fn snapshot(path: &Path) -> Snapshot {
    list(path, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap()
}

fn controller(path: &Path) -> Controller {
    let mut app = App::new(path.into());
    for id in 0..2 {
        assert!(app.complete_listing(id, 1, Ok(snapshot(path))));
    }
    Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}

fn request(c: &mut Controller, action: Action) -> Request {
    let geometry = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 100, 12));
    c.dispatch(action, &geometry).remove(0)
}

fn complete(c: &mut Controller, request: Request) {
    let listing = snapshot(&request.path);
    assert!(
        c.app
            .complete_listing(request.pane, request.generation, Ok(listing))
    );
}

fn navigate(c: &mut Controller, action: Action) {
    let next = request(c, action);
    complete(c, next);
}

fn directories(root: &Path) {
    for index in 0..40 {
        std::fs::create_dir(root.join(format!("child{index:02}"))).unwrap();
    }
}

#[test]
fn parent_and_back_restore_the_exited_child_and_saved_viewport() {
    for action in [Action::Parent, Action::Back] {
        let root = tempfile::tempdir().unwrap();
        directories(root.path());
        let mut c = controller(root.path());
        c.app.panes[0].move_cursor(30, 5);
        let child = root.path().join("child30");
        assert_eq!(c.app.panes[0].cursor.as_ref(), Some(&child));
        assert_eq!(c.app.panes[0].scroll, 26);
        navigate(&mut c, Action::Enter);
        navigate(&mut c, action);
        let pane = &c.app.panes[0];
        assert_eq!(pane.cursor.as_ref(), Some(&child));
        assert_eq!(pane.scroll, 26);
        assert_eq!(pane.viewport_start(5), 26);
        assert_eq!(pane.viewport_start(1), 30);
        assert_eq!(c.app.panes[1].cursor, Some(root.path().join("child00")));
    }
}

#[test]
fn parent_from_direct_start_selects_the_exited_directory() {
    let root = tempfile::tempdir().unwrap();
    directories(root.path());
    let child = root.path().join("child30");
    let mut c = controller(&child);
    navigate(&mut c, Action::Parent);
    assert_eq!(c.app.panes[0].cursor, Some(child));
    assert_eq!(c.app.panes[0].viewport_start(5), 26);
}

#[test]
fn parent_restoration_follows_native_path_after_sorted_contents_shift() {
    let root = tempfile::tempdir().unwrap();
    directories(root.path());
    let mut c = controller(root.path());
    c.app.panes[0].sort.reverse = true;
    c.app.panes[0].resort();
    c.app.panes[0].move_cursor(-isize::MAX, 5);
    c.app.panes[0].move_cursor(30, 5);
    let child = root.path().join("child09");
    assert_eq!(c.app.panes[0].cursor.as_ref(), Some(&child));
    navigate(&mut c, Action::Enter);
    std::fs::remove_dir(root.path().join("child39")).unwrap();
    navigate(&mut c, Action::Parent);
    assert_eq!(c.app.panes[0].cursor, Some(child));
    assert_eq!(c.app.panes[0].cursor_index(), Some(29));
    assert_eq!(c.app.panes[0].scroll, 26);
    assert_eq!(c.app.panes[0].viewport_start(2), 28);
}

#[test]
fn nested_parent_history_and_unrelated_back_forward_remain_distinct() {
    let root = tempfile::tempdir().unwrap();
    directories(root.path());
    let child = root.path().join("child30");
    let grandchild = child.join("nested");
    std::fs::create_dir(&grandchild).unwrap();
    let mut c = controller(root.path());
    c.app.panes[0].move_cursor(30, 5);
    navigate(&mut c, Action::Enter);
    navigate(&mut c, Action::Enter);
    navigate(&mut c, Action::Parent);
    assert_eq!(c.app.panes[0].cursor, Some(grandchild.clone()));
    navigate(&mut c, Action::Parent);
    assert_eq!(c.app.panes[0].cursor, Some(child.clone()));
    navigate(&mut c, Action::Back);
    assert_eq!(c.app.panes[0].path, child);
    navigate(&mut c, Action::Back);
    assert_eq!(c.app.panes[0].path, grandchild);
    navigate(&mut c, Action::Forward);
    assert_eq!(c.app.panes[0].cursor, Some(grandchild));
    navigate(&mut c, Action::Forward);
    assert_eq!(c.app.panes[0].cursor, Some(child));
    assert_eq!(c.app.panes[0].scroll, 26);
    let other = tempfile::tempdir().unwrap();
    let jump = c.app.request_listing(0, other.path().into());
    complete(&mut c, jump);
    navigate(&mut c, Action::Back);
    assert_eq!(c.app.panes[0].path, root.path());
    assert_eq!(c.app.panes[0].scroll, 26);
    navigate(&mut c, Action::Forward);
    assert_eq!(c.app.panes[0].path, other.path());
}

#[test]
fn failed_and_superseded_parent_completions_do_not_consume_history() {
    let root = tempfile::tempdir().unwrap();
    directories(root.path());
    let mut c = controller(root.path());
    c.app.panes[0].move_cursor(30, 5);
    navigate(&mut c, Action::Enter);
    let failed = request(&mut c, Action::Parent);
    assert!(
        c.app
            .complete_listing(0, failed.generation, Err("denied".into()))
    );
    assert_eq!(c.app.panes[0].path, root.path().join("child30"));
    let stale = request(&mut c, Action::Parent);
    let back = request(&mut c, Action::Back);
    assert!(
        !c.app
            .complete_listing(0, stale.generation, Ok(snapshot(root.path())))
    );
    complete(&mut c, back);
    assert_eq!(c.app.panes[0].cursor, Some(root.path().join("child30")));
    assert_eq!(c.app.panes[0].scroll, 26);
    navigate(&mut c, Action::Forward);
    let stale = request(&mut c, Action::Parent);
    let other = tempfile::tempdir().unwrap();
    std::fs::write(other.path().join("other"), "").unwrap();
    let jump = c.app.request_listing(0, other.path().into());
    complete(&mut c, jump);
    assert!(
        !c.app
            .complete_listing(0, stale.generation, Err("late".into()))
    );
    assert_eq!(c.app.panes[0].cursor, Some(other.path().join("other")));
    assert!(c.app.panes[0].error.is_none());
}

#[test]
fn parent_does_not_restore_removed_replaced_or_unidentified_children_or_marks() {
    for replacement in [false, true] {
        let root = tempfile::tempdir().unwrap();
        directories(root.path());
        let child = root.path().join("child30");
        let mut c = controller(root.path());
        c.app.panes[0].move_cursor(30, 5);
        c.app.panes[0].toggle_mark();
        navigate(&mut c, Action::Enter);
        std::fs::rename(&child, root.path().join("old-child")).unwrap();
        if replacement {
            std::fs::create_dir(&child).unwrap();
        }
        navigate(&mut c, Action::Parent);
        assert_eq!(c.app.panes[0].cursor, Some(root.path().join("child00")));
        assert_eq!(c.app.panes[0].scroll, 0);
        assert!(c.app.panes[0].marks.is_empty());
    }
    let root = tempfile::tempdir().unwrap();
    directories(root.path());
    let mut c = controller(&root.path().join("child30"));
    c.app.panes[0].directory = None;
    navigate(&mut c, Action::Parent);
    assert_eq!(c.app.panes[0].cursor, Some(root.path().join("child00")));
}

#[test]
fn parent_obeys_filters_hidden_ignore_rules_and_empty_listings() {
    use terminal_file_manager::app::IgnoreMode;
    for exclusion in ["filter", "hidden", "ignored", "empty"] {
        let root = tempfile::tempdir().unwrap();
        directories(root.path());
        let mut c = controller(&root.path().join("child30"));
        let next = request(&mut c, Action::Parent);
        let mut listing = snapshot(root.path());
        match exclusion {
            "filter" => c.app.panes[0].set_filter("child00".into()),
            "hidden" => listing.entries.iter_mut().for_each(|entry| {
                entry.hidden = entry.path == root.path().join("child30");
            }),
            "ignored" => {
                c.app.panes[0].ignore = IgnoreMode::Hide;
                listing.entries.iter_mut().for_each(|entry| {
                    entry.ignored = Some(entry.path == root.path().join("child30"));
                });
            }
            _ => listing.entries.clear(),
        }
        assert!(c.app.complete_listing(0, next.generation, Ok(listing)));
        let expected = (exclusion != "empty").then(|| root.path().join("child00"));
        assert_eq!(c.app.panes[0].cursor, expected);
        assert_eq!(c.app.panes[0].scroll, 0);
    }
}

// APFS rejects invalid UTF-8 filenames before navigation can be exercised.
#[cfg(target_os = "linux")]
#[test]
fn parent_selects_native_non_utf8_path_despite_display_name_collision() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join(OsString::from_vec(b"child\xfe".to_vec()));
    let child = root.path().join(OsString::from_vec(b"child\xff".to_vec()));
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&child).unwrap();
    assert_eq!(first.to_string_lossy(), child.to_string_lossy());
    let mut c = controller(&child);
    navigate(&mut c, Action::Parent);
    assert_eq!(c.app.panes[0].cursor, Some(child));
}

#[test]
fn parent_can_reuse_a_parent_view_saved_in_forward_history() {
    let root = tempfile::tempdir().unwrap();
    directories(root.path());
    let child = root.path().join("child30");
    let mut c = controller(&child);
    navigate(&mut c, Action::Parent);
    c.app.panes[0].scroll = 27;
    navigate(&mut c, Action::Back);
    navigate(&mut c, Action::Parent);
    assert_eq!(c.app.panes[0].cursor, Some(child));
    assert_eq!(c.app.panes[0].scroll, 27);
    assert!(c.app.history(true).is_none());
}

#[test]
fn both_parent_keyboard_bindings_restore_the_exited_child() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    for (code, modifiers) in [
        (KeyCode::Backspace, KeyModifiers::NONE),
        (KeyCode::Up, KeyModifiers::ALT),
    ] {
        let root = tempfile::tempdir().unwrap();
        directories(root.path());
        let child = root.path().join("child30");
        let mut c = controller(&child);
        let geometry = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 100, 12));
        let next = c
            .handle(
                Event::Key(KeyEvent::new(code, modifiers)),
                &geometry,
                Default::default(),
            )
            .remove(0);
        complete(&mut c, next);
        assert_eq!(c.app.panes[0].cursor, Some(child));
    }
}

#[test]
fn mismatched_completion_cannot_leak_parent_restoration_into_refresh() {
    let root = tempfile::tempdir().unwrap();
    directories(root.path());
    let child = root.path().join("child30");
    std::fs::write(child.join("entry"), "").unwrap();
    let mut c = controller(&child);
    let next = request(&mut c, Action::Parent);
    assert!(
        !c.app
            .complete_listing(0, next.generation, Ok(snapshot(&child)))
    );
    assert_eq!(c.app.panes[0].cursor, Some(child.join("entry")));
    let refresh = c.app.request_refresh(0).unwrap();
    complete(&mut c, refresh);
    assert_eq!(c.app.panes[0].path, child);
    assert!(c.app.history(false).is_none());
}

#[test]
fn replaced_parent_identity_discards_the_saved_viewport() {
    let root = tempfile::tempdir().unwrap();
    directories(root.path());
    let mut c = controller(root.path());
    c.app.panes[0].move_cursor(30, 5);
    navigate(&mut c, Action::Enter);
    let next = request(&mut c, Action::Parent);
    let mut listing = snapshot(root.path());
    listing
        .directory
        .as_mut()
        .unwrap()
        .identity
        .as_mut()
        .unwrap()
        .object = Some((0, 0));
    assert!(c.app.complete_listing(0, next.generation, Ok(listing)));
    assert_eq!(c.app.panes[0].cursor, Some(root.path().join("child30")));
    assert_eq!(c.app.panes[0].scroll, 0);
    assert_eq!(c.app.panes[0].viewport_start(5), 26);
}

#[cfg(unix)]
#[test]
fn parent_restores_a_link_entry_by_its_saved_no_follow_identity() {
    let root = tempfile::tempdir().unwrap();
    directories(root.path());
    let target = tempfile::tempdir().unwrap();
    let link = root.path().join("link");
    std::os::unix::fs::symlink(target.path(), &link).unwrap();
    let mut c = controller(root.path());
    c.app.panes[0].move_cursor(isize::MAX, 5);
    navigate(&mut c, Action::Enter);
    assert_eq!(c.app.panes[0].path, link);
    navigate(&mut c, Action::Parent);
    assert_eq!(c.app.panes[0].cursor, Some(link.clone()));
    assert_eq!(c.app.panes[0].scroll, 36);
    let enter = request(&mut c, Action::Enter);
    let geometry = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 100, 12));
    c.dispatch(Action::Up, &geometry);
    complete(&mut c, enter);
    navigate(&mut c, Action::Parent);
    assert_eq!(c.app.panes[0].cursor, Some(link.clone()));
    navigate(&mut c, Action::Enter);
    std::fs::rename(&link, root.path().join("old-link")).unwrap();
    std::os::unix::fs::symlink(target.path(), &link).unwrap();
    navigate(&mut c, Action::Parent);
    assert_eq!(c.app.panes[0].cursor, Some(root.path().join("child00")));
    assert_eq!(c.app.panes[0].scroll, 0);
}

#[test]
fn concurrent_parent_requests_restore_each_pane_independently() {
    let root = tempfile::tempdir().unwrap();
    directories(root.path());
    let mut c = controller(root.path());
    c.app.panes[0].move_cursor(30, 5);
    navigate(&mut c, Action::Enter);
    let left = request(&mut c, Action::Parent);
    c.app.active = 1;
    c.app.panes[1].move_cursor(20, 5);
    navigate(&mut c, Action::Enter);
    let right = request(&mut c, Action::Parent);
    complete(&mut c, right);
    complete(&mut c, left);
    assert_eq!(c.app.panes[0].cursor, Some(root.path().join("child30")));
    assert_eq!(c.app.panes[0].scroll, 26);
    assert_eq!(c.app.panes[1].cursor, Some(root.path().join("child20")));
    assert_eq!(c.app.panes[1].scroll, 16);
}

#[test]
fn back_does_not_restore_focus_to_a_replacement_child() {
    let root = tempfile::tempdir().unwrap();
    directories(root.path());
    let child = root.path().join("child30");
    let mut c = controller(root.path());
    c.app.panes[0].move_cursor(30, 5);
    navigate(&mut c, Action::Enter);
    std::fs::rename(&child, root.path().join("old-child")).unwrap();
    std::fs::create_dir(&child).unwrap();
    navigate(&mut c, Action::Back);
    assert_eq!(c.app.panes[0].cursor, Some(root.path().join("child00")));
    assert_eq!(c.app.panes[0].scroll, 0);
    navigate(&mut c, Action::Forward);
    assert_eq!(c.app.panes[0].path, child);
}
