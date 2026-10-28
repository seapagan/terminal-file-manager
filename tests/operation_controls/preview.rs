use super::*;

#[test]
fn changed_destructive_scope_cancels_the_old_preview_before_replacing_it() {
    for (action, confirm) in [(Action::Delete, 'D'), (Action::Trash, 't')] {
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
