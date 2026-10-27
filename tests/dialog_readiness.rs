use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Bindings, Controller},
    operations::{Dialog, Intent, Operation, Plan, Queue, Update},
    ui::{self, OperationTarget},
};
mod dialog_fixture;
fn completed_plan(operation: Operation) -> (tempfile::TempDir, Plan) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("selected");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("descendant"), b"retained").unwrap();
    let plan = Plan::paths(operation, vec![path], None).unwrap();
    let mut queue = Queue::new().unwrap();
    queue.preview(plan).unwrap();
    loop {
        if let Update::Preview { plan, .. } = queue.next(std::time::Duration::from_secs(5)).unwrap()
        {
            return (root, *plan);
        }
    }
}
fn button(g: &ui::Geometry, key: KeyCode) -> &ui::OperationControl {
    g.operation_controls
        .iter()
        .find(|c| c.target == OperationTarget::Key(key))
        .unwrap()
}
fn click(c: &mut Controller, g: &ui::Geometry, area: Rect) {
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }),
        g,
        std::time::Duration::ZERO,
    );
}
#[test]
fn pending_and_completed_preview_keep_the_same_buttons_with_true_disabled_semantics() {
    for operation in [Operation::Trash, Operation::Delete] {
        let (_root, plan) = completed_plan(operation);
        let preview = plan.preview.clone();
        let mut app = dialog_fixture::dialog(plan);
        if let Some(Dialog::Operation(d)) = &mut app.operations.dialog {
            d.ready = false;
            d.plan.preview = None;
        }
        let mut c = Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        );
        let key = if operation == Operation::Trash {
            KeyCode::Char('T')
        } else {
            KeyCode::Char('D')
        };
        let pending = ui::geometry(&c.app, Rect::new(0, 0, 80, 30));
        let disabled = button(&pending, key);
        assert!(!disabled.enabled);
        assert_eq!(
            pending.operation_target(disabled.area.x, disabled.area.y),
            None
        );
        click(&mut c, &pending, disabled.area);
        c.handle(
            Event::Key(KeyEvent::new(key, KeyModifiers::NONE)),
            &pending,
            std::time::Duration::ZERO,
        );
        assert!(c.app.operations.outbox.is_empty());
        assert_disabled_buffer(&c.app, disabled.area);
        install_preview(&mut c, preview);
        let ready = ui::geometry(&c.app, Rect::new(0, 0, 80, 30));
        assert!(button(&ready, key).enabled);
        assert_eq!(button(&ready, key).area, disabled.area);
        assert_eq!(button(&ready, key).label, disabled.label);
        assert_failed_transition(&mut c, key, disabled.area);
        click(&mut c, &ready, button(&ready, key).area);
        assert!(
            c.app
                .operations
                .outbox
                .iter()
                .any(|i| matches!(i, Intent::Submit(..)))
        );
    }
}
fn install_preview(
    c: &mut Controller,
    preview: Option<terminal_file_manager::operations::Preview>,
) {
    if let Some(Dialog::Operation(d)) = &mut c.app.operations.dialog {
        d.ready = true;
        d.plan.preview = preview;
    }
}

fn assert_failed_transition(c: &mut Controller, key: KeyCode, expected: Rect) {
    if let Some(Dialog::Operation(d)) = &mut c.app.operations.dialog {
        d.ready = false;
        d.error = Some("Access denied".into());
    }
    let failed = ui::geometry(&c.app, Rect::new(0, 0, 80, 30));
    assert_eq!(button(&failed, key).area, expected);
    assert!(!button(&failed, key).enabled);
    click(c, &failed, expected);
    assert!(c.app.operations.outbox.is_empty());
    if let Some(Dialog::Operation(d)) = &mut c.app.operations.dialog {
        d.ready = true;
        d.error = None;
    }
}

fn assert_disabled_buffer(app: &App, area: Rect) {
    let theme = Theme::new(colored_text::ColorLevel::NoColor);
    let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, app, &theme);
        })
        .unwrap();
    assert_eq!(
        terminal.backend().buffer()[(area.x, area.y)].modifier,
        theme.style(Role::Disabled).add_modifier
    );
}

#[test]
fn completed_mixed_roots_remain_approvable_while_paging_without_submitting() {
    let root = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for index in 0..30 {
        let path = root.path().join(format!("file-{index:02}"));
        std::fs::write(&path, b"data").unwrap();
        paths.push(path);
    }
    let directory = root.path().join("directory");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("descendant"), b"child").unwrap();
    paths.push(directory);
    #[cfg(unix)]
    {
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&paths[0], &link).unwrap();
        paths.push(link);
    }
    let mut queue = Queue::new().unwrap();
    queue
        .preview(Plan::paths(Operation::Delete, paths.clone(), None).unwrap())
        .unwrap();
    let plan = loop {
        if let Update::Preview { plan, .. } = queue.next(std::time::Duration::from_secs(5)).unwrap()
        {
            break *plan;
        }
    };
    let mut c = Controller::new(
        dialog_fixture::dialog(plan),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    for _ in 0..5 {
        let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
        assert!(g.operation_enabled(KeyCode::Char('D')));
        c.handle(
            Event::Key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
            &g,
            std::time::Duration::ZERO,
        );
        assert!(c.app.operations.outbox.is_empty());
        assert!(
            matches!(&c.app.operations.dialog,Some(Dialog::Operation(d)) if d.plan.operands.len()==paths.len())
        );
    }
    assert!(c.app.operations.scroll > 0);
}
#[test]
fn failed_preview_and_clipped_context_block_mouse_and_all_destructive_keys() {
    for operation in [Operation::Trash, Operation::Delete] {
        let (_root, plan) = completed_plan(operation);
        for (width, height, failed) in [
            (20, 8, false),
            (80, 7, false),
            (80, 30, true),
            (1, 20, false),
        ] {
            let mut app = dialog_fixture::dialog(plan.clone());
            if failed && let Some(Dialog::Operation(d)) = &mut app.operations.dialog {
                d.ready = false;
                d.error = Some("Access denied while previewing".into());
            }
            let mut c = Controller::new(
                app,
                Bindings::new(&Default::default()).unwrap(),
                Default::default(),
            );
            let g = ui::geometry(&c.app, Rect::new(0, 0, width, height));
            for control in g
                .operation_controls
                .iter()
                .filter(|c| matches!(c.target, OperationTarget::Key(KeyCode::Char('D' | 'T'))))
            {
                assert!(!control.enabled);
                click(&mut c, &g, control.area);
            }
            for key in [KeyCode::Char('D'), KeyCode::Char('T'), KeyCode::Char('t')] {
                c.handle(
                    Event::Key(KeyEvent::new(key, KeyModifiers::NONE)),
                    &g,
                    std::time::Duration::ZERO,
                );
            }
            assert!(c.app.operations.outbox.is_empty());
            c.handle(
                Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                &g,
                std::time::Duration::ZERO,
            );
            assert!(c.app.operations.dialog.is_none());
            assert!(
                !c.app
                    .operations
                    .outbox
                    .iter()
                    .any(|i| matches!(i, Intent::Submit(..)))
            );
        }
    }
}
