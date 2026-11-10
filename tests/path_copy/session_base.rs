use super::{click_choice, controller, send, sequence};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::{path::Path, sync::atomic::AtomicBool};
use terminal_file_manager::{fs, input::Controller, ui};

#[test]
fn relative_alone_opens_base_stage_and_cancel_preserves_clipboard() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    sequence(&mut c, "ycr");
    assert!(c.path_copy_request.is_none());
    assert_eq!(
        c.app.footer_workflow.as_ref().unwrap().stage().label,
        "Base"
    );
    send(&mut c, KeyCode::Esc);
    assert!(c.app.footer_workflow.is_none());
    assert!(c.path_copy_request.is_none());
}

fn navigate(c: &mut Controller, pane: usize, path: &Path) {
    let request = c.app.request_listing(pane, path.into());
    let listing = fs::list(path, &Default::default(), &AtomicBool::new(false)).unwrap();
    assert!(
        c.app
            .complete_listing(pane, request.generation, Ok(listing))
    );
}

#[test]
fn captured_bases_persist_across_navigation_until_replaced_or_reset() {
    let root = tempfile::tempdir().unwrap();
    let left = root.path().join("left");
    let right = root.path().join("right");
    std::fs::create_dir(&left).unwrap();
    std::fs::create_dir(&right).unwrap();
    let mut c = controller(root.path());
    c.app.dual = true;
    c.app.active = 1;
    c.app.maximized = true;
    navigate(&mut c, 1, &right);
    sequence(&mut c, "ycrs");
    assert_eq!(c.path_copy_request.take().unwrap().text, ".");
    assert_eq!(c.app.relative_path_base, right);
    navigate(&mut c, 1, &left);
    sequence(&mut c, "ycrk");
    assert_eq!(
        c.path_copy_request.take().unwrap().text,
        Path::new("../left")
            .to_str()
            .unwrap()
            .replace('/', std::path::MAIN_SEPARATOR_STR)
    );
    assert_eq!(c.app.relative_path_base, right);
    sequence(&mut c, "ycr");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 40, 24));
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::SHIFT)),
        &g,
        Default::default(),
    );
    assert!(c.path_copy_request.take().is_some());
    assert_eq!(c.app.relative_path_base, root.path());
    navigate(&mut c, 0, &right);
    sequence(&mut c, "ycrk");
    assert_eq!(c.app.relative_path_base, root.path());
    c.path_copy_request.take();
    sequence(&mut c, "ycrs");
    assert_eq!(c.app.relative_path_base, left);
    c.path_copy_request.take();
    sequence(&mut c, "ycro");
    assert_eq!(c.app.relative_path_base, root.path());
    assert!(c.path_copy_request.take().is_some());
    assert!(!c.preferences_changed);
}

#[test]
fn base_stage_back_cancel_and_shift_label_preserve_session_intent() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let mut c = controller(root.path());
    c.app.dual = true;
    navigate(&mut c, 1, &child);
    sequence(&mut c, "ycr");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 40, 24));
    let shifted = g
        .workflow
        .as_ref()
        .unwrap()
        .controls
        .iter()
        .find(|control| {
            control.target
                == Some(terminal_file_manager::workflow::Target::Choice(
                    "opposite-base",
                ))
        })
        .unwrap();
    assert_eq!(shifted.label, "[Shift+s]");
    send(&mut c, KeyCode::Down);
    send(&mut c, KeyCode::Esc);
    assert_eq!(c.app.relative_path_base, root.path());
    assert!(c.path_copy_request.is_none());
    sequence(&mut c, "ycr");
    send(&mut c, KeyCode::Backspace);
    assert_eq!(
        c.app
            .footer_workflow
            .as_ref()
            .unwrap()
            .highlighted()
            .unwrap()
            .id,
        "relative"
    );
    sequence(&mut c, "f");
    assert!(c.path_copy_request.take().is_some());
}

#[test]
fn mouse_capture_persists_and_stale_or_disabled_opposite_capture_cannot_change_base() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let mut c = controller(root.path());
    c.app.dual = true;
    navigate(&mut c, 1, &child);
    let bounds = Rect::new(0, 0, 120, 24);
    sequence(&mut c, "ycr");
    navigate(&mut c, 1, root.path());
    sequence(&mut c, "S");
    assert!(c.path_copy_request.is_none());
    assert_eq!(c.app.relative_path_base, root.path());
    navigate(&mut c, 1, &child);
    for target in ["current", "relative", "opposite-base"] {
        if target == "current" {
            sequence(&mut c, "y");
        }
        click_choice(&mut c, target, bounds);
    }
    assert_eq!(c.app.relative_path_base, child);
    assert!(c.path_copy_request.take().is_some());
    c.app.dual = false;
    sequence(&mut c, "ycrS");
    assert!(c.path_copy_request.is_none());
    assert_eq!(
        c.app.footer_workflow.as_ref().unwrap().stage().label,
        "Base"
    );
    send(&mut c, KeyCode::Enter);
    assert!(c.path_copy_request.take().is_some());
    assert_eq!(c.app.relative_path_base, child);
}

#[cfg(windows)]
#[test]
fn incomparable_saved_base_does_not_block_capturing_this_pane() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    let other_drive = if root.path().to_string_lossy().starts_with('Z') {
        r"Y:\base"
    } else {
        r"Z:\base"
    };
    c.app.relative_path_base = other_drive.into();
    sequence(&mut c, "ycr");
    assert_eq!(
        c.app
            .footer_workflow
            .as_ref()
            .unwrap()
            .highlighted()
            .unwrap()
            .id,
        "current-base"
    );
    send(&mut c, KeyCode::Enter);
    assert_eq!(c.path_copy_request.take().unwrap().text, ".");
    assert_eq!(c.app.relative_path_base, root.path());
}

#[test]
fn copy_path_header_and_from_stage_ignore_old_navigation_keys() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    sequence(&mut c, "y");
    let flow = c.app.footer_workflow.as_ref().unwrap();
    assert_eq!(flow.label, "Copy Path");
    assert_eq!(flow.stage().label, "From");
    for key in [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Tab,
        KeyCode::BackTab,
    ] {
        send(&mut c, key);
        assert_eq!(c.app.footer_workflow.as_ref().unwrap().stage_index(), 0);
        assert_eq!(c.app.panes[0].path, root.path());
        assert_eq!(c.app.active, 0);
    }
    send(&mut c, KeyCode::Backspace);
    assert!(c.app.footer_workflow.is_none());
    assert!(c.path_copy_request.is_none());
}
