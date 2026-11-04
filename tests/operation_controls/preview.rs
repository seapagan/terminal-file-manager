use super::*;

#[test]
fn successful_trash_and_delete_previews_do_not_publish_redundant_notifications() {
    for action in [Action::Trash, Action::Delete] {
        let (root, mut c) = controller();
        c.app.notify(
            terminal_file_manager::notifications::Severity::Warning,
            "Retained warning",
        );
        let plan = pending_preview(&mut c, action, 41);
        preview_dialog(&mut c).ready = false;
        let mut queue = terminal_file_manager::operations::Queue::new().unwrap();
        let id = queue.preview(plan).unwrap();
        preview_dialog(&mut c).preview_id = Some(id);
        let update = preview_result(&mut queue);
        assert!(matches!(update, Update::Preview { .. }));
        c.complete_operation(update);
        assert!(preview_dialog(&mut c).ready);
        assert_eq!(
            preview_dialog(&mut c).plan.preview.as_ref().unwrap().files,
            1
        );
        assert_eq!(c.app.notifications.history.len(), 1);
        assert_eq!(c.app.notifications.history[0].text, "Retained warning");
        assert!(c.app.operations.history.is_empty());
        key(&mut c, KeyCode::Enter);
        assert!(c.app.operations.dialog.is_none());
        assert_eq!(fs::read(root.path().join("left/one")).unwrap(), b"one");
    }
}

fn preview_result(queue: &mut terminal_file_manager::operations::Queue) -> Update {
    loop {
        let update = queue.next(Duration::from_secs(5)).unwrap();
        if !matches!(update, Update::Started { .. }) {
            return update;
        }
    }
}

#[test]
fn failed_destructive_previews_retain_error_notifications_and_disable_confirmation() {
    for action in [Action::Trash, Action::Delete] {
        let (root, mut c) = controller();
        let plan = pending_preview(&mut c, action, 41);
        fs::remove_file(root.path().join("left/one")).unwrap();
        let mut queue = terminal_file_manager::operations::Queue::new().unwrap();
        let id = queue.preview(plan).unwrap();
        preview_dialog(&mut c).preview_id = Some(id);
        c.complete_operation(preview_result(&mut queue));
        assert!(!preview_dialog(&mut c).ready);
        assert!(preview_dialog(&mut c).error.is_some());
        assert_eq!(c.app.notifications.history.len(), 1);
        assert_eq!(
            c.app.notifications.history[0].severity,
            terminal_file_manager::notifications::Severity::Error
        );
        assert!(
            c.app.notifications.history[0]
                .text
                .contains("confirmation disabled")
        );
        assert!(c.app.operations.history.is_empty());
        assert!(c.app.operations.outbox.is_empty());
        assert_eq!(fs::read(root.path().join("left/two")).unwrap(), b"two");
    }
}

#[test]
fn changed_destructive_scope_cancels_the_old_preview_before_replacing_it() {
    for (action, confirm) in [(Action::Delete, 'D'), (Action::Trash, 'T')] {
        let (root, mut controller) = controller();
        controller.app.panes[0].select_all();
        let mut queue = terminal_file_manager::operations::Queue::new().unwrap();
        queue
            .preview(pending_preview(&mut controller, action, 41))
            .unwrap();
        let old = loop {
            if let Update::Preview { plan, .. } = queue.next(Duration::from_secs(5)).unwrap() {
                break *plan;
            }
        };
        preview_dialog(&mut controller).plan = old.clone();
        controller.app.panes[0].set_filter("two".into());
        key(&mut controller, KeyCode::Char(confirm));
        assert!(matches!(
            controller.app.operations.outbox.first(),
            Some(Intent::Cancel(41))
        ));
        assert_eq!(controller.app.operations.outbox.len(), 2);
        assert!(matches!(
            controller.app.operations.outbox.last(),
            Some(Intent::Preview(_))
        ));
        controller.complete_operation(Update::Preview {
            id: 41,
            plan: Box::new(old),
        });
        let dialog = preview_dialog(&mut controller);
        assert!(!dialog.ready);
        assert_eq!(dialog.plan.operands.len(), 1);
        assert_eq!(dialog.plan.operands[0].name, "two");
        key(&mut controller, KeyCode::Char(confirm));
        assert_eq!(controller.app.operations.outbox.len(), 2);
        key(&mut controller, KeyCode::Esc);
        assert!(
            controller
                .app
                .operations
                .outbox
                .iter()
                .all(|intent| matches!(intent, Intent::Cancel(_)))
        );
        assert_eq!(fs::read(root.path().join("left/one")).unwrap(), b"one");
        assert_eq!(fs::read(root.path().join("left/two")).unwrap(), b"two");
    }
}

fn preview_dialog(
    controller: &mut Controller,
) -> &mut terminal_file_manager::operations::OperationDialog {
    match controller.app.operations.dialog.as_mut().unwrap() {
        terminal_file_manager::operations::Dialog::Operation(dialog) => dialog,
        _ => panic!("operation dialog"),
    }
}

fn pending_preview(
    controller: &mut Controller,
    action: Action,
    id: u64,
) -> terminal_file_manager::operations::Plan {
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(action, &geometry);
    controller.app.operations.outbox.clear();
    let dialog = preview_dialog(controller);
    dialog.preview_id = Some(id);
    dialog.ready = true;
    dialog.plan.clone()
}

#[test]
fn interrupt_cancels_a_preview_before_entering_safe_exit() {
    use terminal_file_manager::operations::Dialog;
    let (_, mut controller) = controller();
    running(&mut controller);
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    pending_preview(&mut controller, Action::Delete, 42);
    controller.handle(
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        &geometry,
        Duration::ZERO,
    );
    assert!(matches!(
        controller.app.operations.outbox.first(),
        Some(Intent::Cancel(42))
    ));
    assert!(matches!(
        controller.app.operations.dialog,
        Some(Dialog::Exit { .. })
    ));
    assert!(!controller.quit);
}
