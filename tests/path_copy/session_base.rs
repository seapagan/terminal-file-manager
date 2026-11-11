use super::{click_choice, controller, finish_base, send, sequence};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::{path::Path, sync::atomic::AtomicBool};
use terminal_file_manager::{fs, input::Controller, ui};

fn navigate(c: &mut Controller, pane: usize, path: &Path) {
    let request = c.app.request_listing(pane, path.into());
    let listing = fs::list(path, &Default::default(), &AtomicBool::new(false)).unwrap();
    assert!(
        c.app
            .complete_listing(pane, request.generation, Ok(listing))
    );
}

#[test]
fn relative_completes_at_format_and_base_setting_completes_at_from() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    sequence(&mut c, "ycr");
    assert_eq!(c.path_copy_request.take().unwrap().prepare().unwrap(), ".");
    assert!(c.app.footer_workflow.is_none());
    sequence(&mut c, "ys");
    assert!(c.path_copy_request.is_none());
    assert!(c.app.footer_workflow.is_none());
    assert_eq!(c.app.relative_path_base, root.path());
    assert!(c.app.status.to_string().contains("Relative-path base set"));
}

#[test]
fn captured_bases_persist_across_navigation_without_copying_or_changing_preferences() {
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
    sequence(&mut c, "ys");
    assert_eq!(c.app.relative_path_base, right);
    assert!(c.path_copy_request.is_none());
    navigate(&mut c, 1, &left);
    sequence(&mut c, "ycr");
    assert_eq!(
        c.path_copy_request.take().unwrap().prepare().unwrap(),
        Path::new("..").join("left").to_str().unwrap()
    );
    sequence(&mut c, "y");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 40, 24));
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::SHIFT)),
        &g,
        Default::default(),
    );
    finish_base(&mut c);
    assert!(c.app.footer_workflow.is_none());
    assert!(c.path_copy_request.is_none());
    assert_eq!(c.app.relative_path_base, root.path());
    navigate(&mut c, 0, &right);
    sequence(&mut c, "ycr");
    assert_eq!(
        c.path_copy_request.take().unwrap().prepare().unwrap(),
        "left"
    );
    assert_eq!(c.app.relative_path_base, root.path());
    assert!(!c.preferences_changed);
}

#[test]
fn only_from_and_format_choices_exist_and_formats_complete_immediately() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    for format in ['n', 'r', 'f'] {
        sequence(&mut c, "y");
        let stage = c.app.footer_workflow.as_ref().unwrap().stage();
        assert_eq!(
            stage
                .choices
                .iter()
                .map(|choice| choice.key)
                .collect::<Vec<_>>(),
            ['m', 'h', 'c', 's', 'S']
        );
        sequence(&mut c, "c");
        let flow = c.app.footer_workflow.as_ref().unwrap();
        assert_eq!(flow.stage_index(), 1);
        assert_eq!(
            flow.stage()
                .choices
                .iter()
                .map(|choice| choice.key)
                .collect::<Vec<_>>(),
            ['n', 'r', 'f']
        );
        send(&mut c, KeyCode::Char(format));
        assert!(c.app.footer_workflow.is_none());
        assert!(c.path_copy_request.take().is_some());
    }
}

#[test]
fn mouse_base_capture_and_backtracking_set_a_base_without_a_clipboard_request() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let mut c = controller(root.path());
    c.app.dual = true;
    navigate(&mut c, 1, &child);
    let bounds = Rect::new(0, 0, 160, 24);
    sequence(&mut c, "y");
    click_choice(&mut c, "opposite-base", bounds);
    assert!(c.app.footer_workflow.is_none());
    assert!(c.path_copy_request.is_none());
    assert_eq!(c.app.relative_path_base, child);
    assert!(c.app.status.to_string().contains("Relative-path base set"));
    sequence(&mut c, "yc");
    send(&mut c, KeyCode::Backspace);
    assert_eq!(
        c.app
            .footer_workflow
            .as_ref()
            .unwrap()
            .highlighted()
            .unwrap()
            .id,
        "current"
    );
    click_choice(&mut c, "current-base", bounds);
    assert!(c.app.footer_workflow.is_none());
    assert!(c.path_copy_request.is_none());
    assert_eq!(c.app.relative_path_base, root.path());
}

#[test]
fn unavailable_opposite_base_is_disabled_and_invalid_native_base_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    std::fs::write(&file, "data").unwrap();
    for case in 0..6 {
        let mut c = controller(root.path());
        c.app.dual = true;
        match case {
            0 => c.app.dual = false,
            1 => c.app.panes[1].pending = Some(root.path().into()),
            2 => c.app.panes[1].error = Some("unavailable".into()),
            3 => {
                c.app.panes[1].path = root.path().join("missing");
            }
            4 => {
                c.app.panes[1].path = file.clone();
            }
            _ => c.app.panes[1].path = "relative".into(),
        }
        sequence(&mut c, "yS");
        if matches!(case, 3 | 4) {
            assert!(c.app.footer_workflow.is_none());
        } else {
            assert_other_disabled(&c);
        }
        assert_eq!(c.app.relative_path_base, root.path());
        assert!(c.path_copy_request.is_none());
        send(&mut c, KeyCode::Esc);
    }
}

fn assert_other_disabled(c: &Controller) {
    let flow = c.app.footer_workflow.as_ref().unwrap();
    assert_eq!(flow.stage_index(), 0);
    assert!(
        flow.stage()
            .choices
            .iter()
            .find(|choice| choice.id == "opposite-base")
            .unwrap()
            .disabled
            .is_some()
    );
    let g = ui::geometry(&c.app, Rect::new(0, 0, 160, 24));
    let layout = g.workflow.as_ref().unwrap();
    let control = layout
        .controls
        .iter()
        .find(|control| {
            control.target
                == Some(terminal_file_manager::workflow::Target::Choice(
                    "opposite-base",
                ))
        })
        .unwrap();
    assert_eq!(layout.hit(control.area.x, control.area.y), None);
}

#[test]
fn replaced_or_navigated_opposite_snapshot_cannot_change_the_base() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let mut c = controller(root.path());
    c.app.dual = true;
    navigate(&mut c, 1, &child);
    sequence(&mut c, "y");
    std::fs::rename(&child, root.path().join("old")).unwrap();
    std::fs::create_dir(&child).unwrap();
    sequence(&mut c, "S");
    assert_eq!(c.app.relative_path_base, root.path());
    assert!(c.path_copy_request.is_none());
    navigate(&mut c, 1, &child);
    sequence(&mut c, "y");
    navigate(&mut c, 1, root.path());
    sequence(&mut c, "S");
    assert_eq!(c.app.relative_path_base, root.path());
    assert!(c.path_copy_request.is_none());
}

#[cfg(windows)]
#[test]
fn incomparable_saved_base_disables_relative_but_not_explicit_set_base() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    c.app.relative_path_base = if root.path().to_string_lossy().starts_with('Z') {
        r"Y:\base"
    } else {
        r"Z:\base"
    }
    .into();
    sequence(&mut c, "ycr");
    assert!(c.path_copy_request.is_none());
    assert_eq!(
        c.app.footer_workflow.as_ref().unwrap().stage().label,
        "Format"
    );
    send(&mut c, KeyCode::Backspace);
    sequence(&mut c, "s");
    assert_eq!(c.app.relative_path_base, root.path());
    assert!(c.app.footer_workflow.is_none());
    sequence(&mut c, "ycr");
    assert_eq!(c.path_copy_request.take().unwrap().prepare().unwrap(), ".");
}

#[test]
fn copy_path_header_and_from_stage_ignore_old_navigation_keys() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    sequence(&mut c, "y");
    let flow = c.app.footer_workflow.as_ref().unwrap();
    assert_eq!(flow.label, "Copy Path");
    assert_eq!(flow.stage().label, "From");
    let current = flow
        .stage()
        .choices
        .iter()
        .find(|choice| choice.id == "current")
        .unwrap();
    assert_eq!(current.key, 'c');
    assert_eq!(current.labels, ["Current folder"; 3]);
    let g = ui::geometry(&c.app, Rect::new(0, 0, 160, 24));
    assert!(
        g.workflow
            .as_ref()
            .unwrap()
            .controls
            .iter()
            .any(|control| control.label == "[c Current folder]")
    );
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

#[test]
fn opposite_base_result_cannot_apply_after_late_navigation() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let mut c = controller(root.path());
    c.app.dual = true;
    navigate(&mut c, 1, &child);
    sequence(&mut c, "y");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT)),
        &g,
        Default::default(),
    );
    let validated = c.path_base_request.take().unwrap().prepare().unwrap();
    navigate(&mut c, 1, root.path());
    assert!(validated.apply(&mut c.app).is_err());
    assert_eq!(c.app.relative_path_base, root.path());
    assert!(c.path_copy_request.is_none());
}
