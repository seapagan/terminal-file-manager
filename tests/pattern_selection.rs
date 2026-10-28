//! Pattern selection uses native names and a frozen visible working set.
use std::{fs, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::{App, PatternMode, PatternOperation, PatternSelection},
    fs::{IgnoreContext, list},
};

fn loaded() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    for name in ["alpha.txt", "BETA.TXT", "λ.txt", "keep.log"] {
        fs::write(root.path().join(name), "owned").unwrap();
    }
    let mut hidden = fs::OpenOptions::new();
    hidden.write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        hidden.attributes(2);
    }
    hidden.open(root.path().join(".hidden.txt")).unwrap();
    fs::create_dir(root.path().join("folder.txt")).unwrap();
    let mut app = App::new(root.path().into());
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
    (root, app)
}

#[test]
fn wildcard_regex_case_and_invalid_patterns_have_explicit_results() {
    let (_root, mut app) = loaded();
    let pane = &mut app.panes[0];
    let mut selection = PatternSelection::new(pane, 0);
    assert!(selection.preview(pane).is_err());
    selection.set_expression("*.txt".into());
    assert_eq!(selection.preview(pane).unwrap().matches, 3);
    selection.toggle_case();
    assert_eq!(selection.preview(pane).unwrap().matches, 4);
    selection.set_mode(PatternMode::Regex);
    selection.set_expression("TXT".into());
    assert_eq!(selection.preview(pane).unwrap().matches, 4);
    selection.set_expression("^alpha$".into());
    assert_eq!(selection.preview(pane).unwrap().matches, 0);
    selection.set_expression("[".into());
    assert!(selection.apply(pane, PatternOperation::Mark).is_err());
    assert!(pane.marks.is_empty());
}

#[test]
fn identities_without_object_ids_follow_manual_marking_and_revalidate() {
    let (_root, mut app) = loaded();
    let pane = &mut app.panes[0];
    for entry in &mut pane.entries {
        entry.identity.as_mut().unwrap().object = None;
    }
    pane.entries[0].identity = None;
    pane.select_all();
    let expected = pane.marks.clone();
    pane.clear_visible();
    let mut selection = PatternSelection::new(pane, 0);
    selection.set_expression("*".into());
    assert_eq!(selection.preview(pane).unwrap().matches, expected.len());
    selection.apply(pane, PatternOperation::Mark).unwrap();
    assert_eq!(pane.marks, expected);
    let selected = *pane
        .visible
        .iter()
        .find(|i| pane.entries[**i].identity.is_some())
        .unwrap();
    pane.entries[selected].identity.as_mut().unwrap().length += 1;
    assert!(selection.apply(pane, PatternOperation::Replace).is_err());
    assert_eq!(pane.marks, expected);
}

#[test]
fn replace_and_unmark_preserve_excluded_marks_and_zero_never_marks_cursor() {
    let (_root, mut app) = loaded();
    let pane = &mut app.panes[0];
    pane.show_hidden = true;
    pane.refilter();
    pane.select_all();
    pane.show_hidden = false;
    pane.set_filter("txt".into());
    let mut selection = PatternSelection::new(pane, 0);
    selection.set_expression("missing".into());
    selection.apply(pane, PatternOperation::Mark).unwrap();
    assert_eq!(pane.marks.len(), 6);
    selection.apply(pane, PatternOperation::Replace).unwrap();
    assert_eq!(pane.marks.len(), 2);
    assert!(pane.marks.contains_key(&pane.path.join(".hidden.txt")));
    assert!(pane.marks.contains_key(&pane.path.join("keep.log")));
    selection.set_expression("*.txt".into());
    selection.apply(pane, PatternOperation::Mark).unwrap();
    assert_eq!(pane.marks.len(), 5);
    selection.apply(pane, PatternOperation::Unmark).unwrap();
    assert_eq!(pane.marks.len(), 2);
}

#[test]
fn changed_visible_set_blocks_application_instead_of_expanding_selection() {
    let (root, mut app) = loaded();
    let mut selection = PatternSelection::new(&app.panes[0], 0);
    selection.set_expression("*".into());
    fs::write(root.path().join("arrival"), "new").unwrap();
    let request = app.request_refresh(0).unwrap();
    app.complete_listing(
        0,
        request.generation,
        Ok(list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    assert!(
        selection
            .apply(&mut app.panes[0], PatternOperation::Mark)
            .is_err()
    );
    assert!(app.panes[0].marks.is_empty());
}

#[test]
fn alt_b_modal_owns_input_paste_and_escape_without_changing_marks() {
    use crossterm::event::{Event, KeyEvent, KeyModifiers};
    use terminal_file_manager::{
        input::{Bindings, Controller, chord},
        ui,
    };
    let (_root, app) = loaded();
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let area = ratatui::layout::Rect::new(0, 0, 100, 24);
    let (code, modifiers) = chord("Alt+b").unwrap();
    c.handle(
        Event::Key(KeyEvent::new(code, modifiers)),
        &ui::geometry(&c.app, area),
        std::time::Duration::ZERO,
    );
    assert!(c.app.pattern.is_some());
    c.handle(
        Event::Paste("*.txt".into()),
        &ui::geometry(&c.app, area),
        std::time::Duration::ZERO,
    );
    assert_eq!(c.app.pattern.as_ref().unwrap().expression(), "*.txt");
    c.handle(
        Event::Key(KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            KeyModifiers::NONE,
        )),
        &ui::geometry(&c.app, area),
        std::time::Duration::ZERO,
    );
    assert!(c.app.pattern.is_none());
    assert!(c.app.panes[0].marks.is_empty());
    assert!(c.app.panes[0].filter.is_empty());
}

#[test]
fn sort_and_unchanged_refresh_preserve_captured_identities_and_range_ownership() {
    let (root, mut app) = loaded();
    let mut selection = PatternSelection::new(&app.panes[0], 0);
    selection.set_expression("*.txt".into());
    app.panes[0].sort.reverse = true;
    app.panes[0].resort();
    let request = app.request_refresh(0).unwrap();
    assert!(selection.preview(&app.panes[0]).is_err());
    app.complete_listing(
        0,
        request.generation,
        Ok(list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    selection
        .apply(&mut app.panes[0], PatternOperation::Mark)
        .unwrap();
    let marks = app.panes[0].marks.clone();
    assert_eq!(marks.len(), 3);
    app.panes[0].extend_range(1, 10);
    app.panes[0].extend_range(-1, 10);
    assert!(
        marks
            .iter()
            .all(|(p, id)| app.panes[0].marks.get(p) == Some(id))
    );
}

#[test]
fn hidden_filter_and_replaced_identity_changes_cannot_retarget_approval() {
    let (_root, mut app) = loaded();
    let pane = &mut app.panes[0];
    let mut selection = PatternSelection::new(pane, 0);
    selection.set_expression("*".into());
    pane.set_filter("alpha".into());
    assert!(selection.apply(pane, PatternOperation::Replace).is_err());
    pane.set_filter(String::new());
    pane.show_hidden = true;
    pane.refilter();
    assert!(selection.preview(pane).is_err());
    pane.show_hidden = false;
    pane.refilter();
    pane.entries
        .iter_mut()
        .find(|e| e.name == "alpha.txt")
        .unwrap()
        .identity
        .as_mut()
        .unwrap()
        .object = Some((u64::MAX, u64::MAX));
    assert!(selection.apply(pane, PatternOperation::Mark).is_err());
    assert!(pane.marks.is_empty());
}

#[test]
fn wildcard_is_whole_name_regex_is_unicode_substring_and_zero_is_a_noop() {
    let (_root, mut app) = loaded();
    let pane = &mut app.panes[0];
    let mut selection = PatternSelection::new(pane, 0);
    selection.set_expression("txt".into());
    assert_eq!(selection.preview(pane).unwrap().matches, 0);
    selection.apply(pane, PatternOperation::Mark).unwrap();
    assert!(pane.marks.is_empty());
    selection.set_mode(PatternMode::Regex);
    selection.set_expression("^Λ".into());
    assert_eq!(selection.preview(pane).unwrap().matches, 0);
    selection.toggle_case();
    assert_eq!(selection.preview(pane).unwrap().matches, 1);
    selection.apply(pane, PatternOperation::Mark).unwrap();
    assert!(pane.marks.contains_key(&pane.path.join("λ.txt")));
}

#[cfg(target_os = "linux")]
#[test]
fn native_invalid_bytes_are_distinct_from_lossy_display_names() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let (root, mut app) = loaded();
    let native = OsString::from_vec(b"bad-\xff.txt".to_vec());
    fs::write(root.path().join(&native), "native").unwrap();
    fs::write(root.path().join("bad-�.txt"), "unicode").unwrap();
    let request = app.request_refresh(0).unwrap();
    app.complete_listing(
        0,
        request.generation,
        Ok(list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    let pane = &mut app.panes[0];
    let mut selection = PatternSelection::new(pane, 0);
    selection.set_expression("bad-�.txt".into());
    selection.apply(pane, PatternOperation::Mark).unwrap();
    assert!(!pane.marks.contains_key(&root.path().join(&native)));
    selection.set_expression("bad-*.txt".into());
    selection.apply(pane, PatternOperation::Mark).unwrap();
    assert!(pane.marks.contains_key(&root.path().join(&native)));
    selection.set_mode(PatternMode::Regex);
    selection.set_expression(r"(?-u:\xFF)".into());
    assert_eq!(selection.preview(pane).unwrap().matches, 1);
}

#[cfg(windows)]
#[test]
fn windows_native_surrogate_names_never_match_replacement_display_text() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    let (root, mut app) = loaded();
    let native = OsString::from_wide(&[
        b'b' as u16,
        0xd800,
        b'.' as u16,
        b't' as u16,
        b'x' as u16,
        b't' as u16,
    ]);
    fs::write(root.path().join(&native), "native").unwrap();
    let request = app.request_refresh(0).unwrap();
    app.complete_listing(
        0,
        request.generation,
        Ok(list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    let pane = &mut app.panes[0];
    let mut selection = PatternSelection::new(pane, 0);
    selection.set_expression("b�.txt".into());
    assert_eq!(selection.preview(pane).unwrap().matches, 0);
    selection.set_expression("b*.txt".into());
    selection.apply(pane, PatternOperation::Mark).unwrap();
    assert!(pane.marks.contains_key(&root.path().join(&native)));
}

#[test]
fn ignored_marks_survive_visible_replace_and_unavailable_names_are_not_selectable() {
    use terminal_file_manager::app::IgnoreMode;
    let (_root, mut app) = loaded();
    let pane = &mut app.panes[0];
    pane.select_all();
    pane.entries
        .iter_mut()
        .find(|e| e.name == "keep.log")
        .unwrap()
        .ignored = Some(true);
    pane.entries
        .iter_mut()
        .find(|e| e.name == "alpha.txt")
        .unwrap()
        .identity = None;
    pane.ignore = IgnoreMode::Hide;
    pane.refilter();
    let mut selection = PatternSelection::new(pane, 0);
    selection.set_expression("none".into());
    assert_eq!(selection.preview(pane).unwrap().visible, 3);
    selection.apply(pane, PatternOperation::Replace).unwrap();
    assert!(pane.marks.contains_key(&pane.path.join("keep.log")));
    assert!(!pane.marks.contains_key(&pane.path.join("λ.txt")));
}

#[test]
fn native_replacement_at_the_same_filename_cannot_receive_a_frozen_mark() {
    let (root, mut app) = loaded();
    app.panes[0].set_filter("alpha".into());
    let mut selection = PatternSelection::new(&app.panes[0], 0);
    selection.set_expression("alpha.txt".into());
    fs::rename(
        root.path().join("alpha.txt"),
        root.path().join(".held-original"),
    )
    .unwrap();
    fs::write(root.path().join("alpha.txt"), "replacement").unwrap();
    let request = app.request_refresh(0).unwrap();
    app.complete_listing(
        0,
        request.generation,
        Ok(list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    assert!(
        selection
            .apply(&mut app.panes[0], PatternOperation::Mark)
            .is_err()
    );
    assert!(app.panes[0].marks.is_empty());
}
