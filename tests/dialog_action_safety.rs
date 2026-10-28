use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use terminal_file_manager::{
    input::{Bindings, Controller},
    operations::{Dialog, Intent, Operation, Plan},
    ui::{self, OperationTarget},
};
mod dialog_fixture;
fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}
fn controller(plan: Plan) -> Controller {
    Controller::new(
        dialog_fixture::dialog(plan),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}
#[test]
fn ready_flag_without_completed_preview_never_submits() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("selected");
    std::fs::write(&path, b"keep").unwrap();
    for operation in [Operation::Trash, Operation::Delete] {
        for height in 0..20 {
            let mut c = controller(Plan::paths(operation, vec![path.clone()], None).unwrap());
            let g = ui::geometry(&c.app, Rect::new(0, 0, 80, height));
            for code in [KeyCode::Char('D'), KeyCode::Char('T'), KeyCode::Char('t')] {
                c.handle(key(code), &g, std::time::Duration::ZERO);
                assert!(
                    !c.app
                        .operations
                        .outbox
                        .iter()
                        .any(|i| matches!(i, Intent::Submit(..) | Intent::SubmitRetry(..))),
                    "{operation:?} at {height}"
                );
            }
        }
    }
}
#[test]
fn empty_or_invalid_create_name_never_submits_on_enter_or_click() {
    for text in [
        "",
        ".",
        "..",
        "bad/name",
        "/absolute",
        "name/",
        "./name",
        "bad\0name",
    ] {
        let mut c = controller(Plan::mkdir("/controlled".into()));
        if let Some(Dialog::Operation(d)) = &mut c.app.operations.dialog {
            d.text = text.into();
        }
        let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 30));
        c.handle(key(KeyCode::Enter), &g, std::time::Duration::ZERO);
        if let Some(button) = g
            .operation_controls
            .iter()
            .find(|c| c.target == OperationTarget::Key(KeyCode::Enter))
        {
            c.handle(
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: button.area.x,
                    row: button.area.y,
                    modifiers: KeyModifiers::NONE,
                }),
                &g,
                std::time::Duration::ZERO,
            );
        }
        assert!(
            c.app.status.contains("one directory name"),
            "{}",
            c.app.status
        );
        assert!(c.app.operations.outbox.is_empty(), "{text:?}");
        assert!(c.app.operations.dialog.is_some());
    }
}
