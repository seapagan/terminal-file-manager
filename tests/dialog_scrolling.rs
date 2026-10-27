use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    color::Theme,
    operations::{Dialog, Operation, Plan},
    ui::{self, OperationTarget},
};
mod dialog_fixture;
use dialog_fixture::dialog;

#[test]
fn selected_roots_scroll_below_fixed_context_and_above_fixed_delete_warning() {
    let root = tempfile::tempdir().unwrap();
    let paths: Vec<_> = (0..50)
        .map(|i| {
            let path = root.path().join(format!("folder{i:02}"));
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("descendant"), b"payload").unwrap();
            path
        })
        .collect();
    let mut app = dialog(Plan::paths(Operation::Delete, paths.clone(), None).unwrap());
    let theme = Theme::new(ColorLevel::Ansi256);
    let mut terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
    let mut positions = Vec::new();
    for scroll in [0, usize::MAX] {
        app.operations.scroll = scroll;
        terminal
            .draw(|frame| {
                ui::render(frame, &app, &theme);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rows: Vec<String> = (0..40)
            .map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect())
            .collect();
        positions.push(fixed_positions(&rows));
        if scroll == usize::MAX {
            assert!(rows.iter().any(|row| row.contains("folder49/")));
        }
    }
    assert_eq!(positions[0], positions[1]);
    assert!(
        matches!(&app.operations.dialog, Some(Dialog::Operation(d)) if d.plan.operands.len() == 50 && d.plan.operands.iter().map(|e| &e.path).eq(paths.iter()))
    );
}
fn fixed_positions(rows: &[String]) -> (usize, usize, usize) {
    let top = rows
        .iter()
        .position(|row| row.contains("PERMANENT DELETE —"))
        .unwrap();
    let warning = rows
        .iter()
        .position(|row| row.contains("PERMANENT DELETE cannot be undone."))
        .unwrap();
    let actions = rows
        .iter()
        .position(|row| row.contains("[Enter Cancel]"))
        .unwrap();
    assert!(actions - top < 30);
    assert!(warning < actions);
    assert!(!rows.iter().any(|row| row.contains("descendant")));
    assert!(
        rows.iter()
            .any(|row| row.contains("d  folder") && row.contains('/'))
    );
    assert!(
        rows.iter()
            .any(|row| row.contains("/50") && row.contains("PgUp"))
    );
    let from = rows.iter().position(|row| row.contains("From:")).unwrap();
    (from, warning, actions)
}

#[test]
fn paging_and_wheel_preserve_fields_and_clamp_after_resize() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
    use terminal_file_manager::input::{Bindings, Controller};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    std::fs::write(&path, b"data").unwrap();
    let mut plan = Plan::paths(Operation::Copy, vec![path], Some(root.path().into())).unwrap();
    plan.operands = vec![plan.operands[0].clone(); 500];
    let mut controller = Controller::new(
        dialog(plan),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 80, 40));
    let field = destination_field(&geometry);
    for (index, event) in [
        Event::Key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 40,
            row: 20,
            modifiers: KeyModifiers::NONE,
        }),
    ]
    .into_iter()
    .enumerate()
    {
        controller.handle(event, &geometry, std::time::Duration::ZERO);
        assert_eq!(controller.app.operations.scroll, 8 + 3 * index);
    }
    assert!(controller.app.operations.scroll > 0);
    let next = ui::geometry(&controller.app, Rect::new(0, 0, 80, 40));
    assert_eq!(destination_field(&next), field);
    assert!(
        matches!(&controller.app.operations.dialog,Some(Dialog::Operation(d)) if d.text.is_empty() && d.field == 0 && d.plan.operands.len() == 500)
    );
    controller.app.operations.scroll = usize::MAX;
    let resized = ui::geometry(&controller.app, Rect::new(0, 0, 40, 20));
    controller.handle(
        Event::Key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
        &resized,
        std::time::Duration::ZERO,
    );
    assert_eq!(
        controller.app.operations.scroll,
        resized.operation_scroll_limit
    );
    assert!(controller.app.operations.outbox.is_empty());
    assert!(controller.app.panes[0].marks.is_empty());
}

fn destination_field(geometry: &ui::Geometry) -> Rect {
    geometry
        .operation_controls
        .iter()
        .find(|c| c.target == OperationTarget::Field(0))
        .unwrap()
        .area
}

#[test]
fn tiny_delete_never_accepts_a_hidden_warning_or_invisible_action() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use terminal_file_manager::input::{Bindings, Controller};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    std::fs::write(&path, b"data").unwrap();
    let plan = Plan::paths(Operation::Delete, vec![path], None).unwrap();
    let mut controller = Controller::new(
        dialog(plan),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    for height in [0, 1, 2, 5, 8] {
        let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 20, height));
        assert!(
            !geometry
                .operation_controls
                .iter()
                .any(|c| c.enabled && c.target == OperationTarget::Key(KeyCode::Char('D')))
        );
        controller.handle(
            Event::Key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT)),
            &geometry,
            std::time::Duration::ZERO,
        );
        assert!(controller.app.operations.dialog.is_some());
        assert!(controller.app.operations.outbox.is_empty());
    }
}
