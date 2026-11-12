use super::{click_choice, controller, send, sequence};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::path::Path;
use terminal_file_manager::ui;

#[test]
fn footer_click_activates_copy_and_emits_one_request() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    let bounds = Rect::new(0, 0, 240, 24);
    let g = ui::geometry(&c.app, bounds);
    let control = g
        .footer_controls
        .iter()
        .find(|control| control.action == terminal_file_manager::input::Action::PathCopy)
        .unwrap();
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: control.area.x,
            row: control.area.y,
            modifiers: KeyModifiers::NONE,
        }),
        &g,
        Default::default(),
    );
    assert!(c.app.footer_workflow.is_some());
    assert!(c.path_copy_request.is_none());
    click_choice(&mut c, "current", bounds);
    click_choice(&mut c, "full", bounds);
    assert!(c.app.footer_workflow.is_none());
    assert_eq!(
        c.path_copy_request.take().unwrap().prepare().unwrap(),
        root.path().to_str().unwrap()
    );
    let mut repeated = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
    repeated.kind = crossterm::event::KeyEventKind::Repeat;
    c.handle(
        Event::Key(repeated),
        &ui::geometry(&c.app, bounds),
        Default::default(),
    );
    assert!(c.path_copy_request.is_none());
}

#[test]
fn input_history_overflow_closes_footer_without_queuing_an_action() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    sequence(&mut c, "y");
    for _ in 1..terminal_file_manager::workflow::MAX_INPUTS {
        send(&mut c, KeyCode::Up);
    }
    assert!(c.app.footer_workflow.is_some());
    send(&mut c, KeyCode::Enter);
    assert!(c.app.footer_workflow.is_none());
    assert!(c.path_copy_request.is_none());
    assert!(c.path_base_request.is_none());
    assert!(
        c.app
            .status
            .to_string()
            .contains("input history limit exceeded")
    );
}

#[test]
fn relative_paths_preserve_case_distinctions_without_filesystem_assumptions() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        terminal_file_manager::path_copy::relative(
            &root.path().join("Project/file"),
            &root.path().join("project")
        )
        .unwrap(),
        Path::new("..").join("Project/file")
    );
}

#[test]
fn disabled_choice_mouse_click_reports_existing_reason_and_keeps_input_ownership() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    sequence(&mut c, "y");
    let before = c
        .app
        .footer_workflow
        .as_ref()
        .unwrap()
        .highlighted()
        .unwrap()
        .id;
    let reason = c
        .app
        .footer_workflow
        .as_ref()
        .unwrap()
        .stage()
        .choices
        .iter()
        .find(|choice| choice.id == "opposite-base")
        .unwrap()
        .disabled
        .clone()
        .unwrap();
    click_choice(&mut c, "opposite-base", Rect::new(0, 0, 240, 24));
    assert_eq!(c.app.status.to_string(), reason);
    let flow = c.app.footer_workflow.as_ref().unwrap();
    assert_eq!(flow.stage_index(), 0);
    assert_eq!(flow.highlighted().unwrap().id, before);
    assert!(!c.quit);
    assert!(c.path_copy_request.is_none());
    assert!(c.path_base_request.is_none());
}
