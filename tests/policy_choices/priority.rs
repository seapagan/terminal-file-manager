use super::*;
use std::{fs, sync::mpsc};
use terminal_file_manager::operations::{Decision, Dialog, Event as JobEvent, Plan, Update};

fn open_sort(c: &mut Controller, bounds: Rect) {
    let g = ui::geometry(&c.app, bounds);
    let control = g
        .policy_controls
        .iter()
        .find(|c| c.action == Action::Sort)
        .unwrap();
    click(c, control.area.x, control.area.y, bounds);
}

fn assert_operation_visible(c: &Controller, title: &str) -> ui::Geometry {
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    let mut geometry = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
    terminal
        .draw(|f| geometry = ui::render(f, &c.app, &Theme::new(colored_text::ColorLevel::NoColor)))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(text.contains(title), "{text}");
    assert!(!text.contains("Choose — Esc cancels"), "{text}");
    assert!(geometry.policy_menu.is_none());
    geometry
}

#[test]
fn operation_dialog_preempts_an_open_policy_menu_and_receives_cancel() {
    let mut c = controller();
    let bounds = Rect::new(0, 0, 120, 30);
    open_sort(&mut c, bounds);
    c.app.operations.dialog = Some(Dialog::Exit { save: false });
    assert_operation_visible(&c, "Active jobs — safe exit");
    assert_eq!(
        terminal_file_manager::input::HelpTopic::current(&c.app),
        Some(terminal_file_manager::input::HelpTopic::Exit)
    );
    key(&mut c, "Esc", bounds);
    assert!(c.app.operations.dialog.is_none());
    assert!(c.app.policy_menu.is_none());
    assert_eq!(c.app.panes[0].sort.sort, Sort::Natural);
    assert!(!c.preferences_changed);
}

#[test]
fn policy_click_cannot_open_a_menu_over_an_operation_dialog() {
    let mut c = controller();
    let bounds = Rect::new(0, 0, 120, 30);
    c.app.operations.dialog = Some(Dialog::Exit { save: false });
    open_sort(&mut c, bounds);
    assert!(c.app.policy_menu.is_none());
    assert!(c.app.operations.dialog.is_some());
    for shortcut in ["S", "G", "H", "I"] {
        key(&mut c, shortcut, bounds);
        assert!(c.app.policy_menu.is_none());
        assert!(c.app.operations.dialog.is_some());
    }
}

fn conflict(c: &mut Controller, root: &std::path::Path) -> mpsc::Receiver<Decision> {
    fs::write(root.join("source"), "incoming").unwrap();
    fs::write(root.join("target"), "existing").unwrap();
    c.register_operation(
        1,
        Plan::mkdir(root.join("unused")),
        Default::default(),
        None,
    );
    let (reply, response) = mpsc::sync_channel(1);
    c.complete_operation(Update::Conflict {
        id: 1,
        event: JobEvent::Conflict {
            source: Box::new(terminal_file_manager::fs::inspect(&root.join("source"))),
            destination: Box::new(terminal_file_manager::fs::inspect(&root.join("target"))),
            newer_available: true,
        },
        reply,
    });
    response
}

#[test]
fn asynchronous_conflict_preempts_policy_keyboard_and_mouse_input() {
    let bounds = Rect::new(0, 0, 120, 30);
    for activation in ["mouse", "s", "S"] {
        let root = tempfile::tempdir().unwrap();
        let mut c = controller();
        open_sort(&mut c, bounds);
        key(&mut c, "End", bounds);
        let response = conflict(&mut c, root.path());
        for shortcut in ["G", "H", "I"] {
            key(&mut c, shortcut, bounds);
            assert!(c.app.policy_menu.is_none());
            assert!(matches!(
                response.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
        }
        let g = assert_operation_visible(&c, "File already exists");
        if activation == "mouse" {
            let skip = g
                .operation_controls
                .iter()
                .find(|c| {
                    c.target == ui::OperationTarget::Key(crossterm::event::KeyCode::Char('s'))
                })
                .unwrap();
            click(&mut c, skip.area.x, skip.area.y, bounds);
        } else {
            key(&mut c, activation, bounds);
        }
        assert!(matches!(response.try_recv().unwrap(), Decision::Skip));
        assert!(c.app.policy_menu.is_none());
        assert_eq!(c.app.panes[0].sort.sort, Sort::Natural);
        assert!(!c.preferences_changed);
        assert_eq!(fs::read(root.path().join("target")).unwrap(), b"existing");
    }
}
