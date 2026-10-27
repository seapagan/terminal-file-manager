use colored_text::ColorLevel;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Bindings, Controller},
    operations::{Dialog, Intent, Operation, Plan},
    ui::{self, OperationTarget},
};
mod dialog_fixture;
fn draw(app: &App, level: ColorLevel) -> (Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(level));
        })
        .unwrap();
    (
        terminal.backend().buffer().clone(),
        ui::geometry(app, Rect::new(0, 0, 120, 40)),
    )
}
fn at(buffer: &Buffer, text: &str) -> (u16, u16) {
    (0..buffer.area.height)
        .find_map(|y| {
            let line: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            line.find(text)
                .map(|start| (ratatui::text::Line::raw(&line[..start]).width() as u16, y))
        })
        .unwrap()
}
#[test]
fn create_requires_a_valid_name_without_stealing_field_focus() {
    let app = dialog_fixture::dialog(Plan::mkdir("/controlled".into()));
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let empty = ui::geometry(&c.app, Rect::new(0, 0, 120, 40));
    assert!(!empty.operation_enabled(KeyCode::Enter));
    c.handle(
        Event::Paste("資料e\u{301}".into()),
        &empty,
        std::time::Duration::ZERO,
    );
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        assert_create_style(&c.app, level);
    }
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 40));
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &g,
        std::time::Duration::ZERO,
    );
    assert!(
        c.app
            .operations
            .outbox
            .iter()
            .any(|i| matches!(i, Intent::Submit(..)))
    );
}
fn assert_create_style(app: &App, level: ColorLevel) {
    let (buffer, g) = draw(app, level);
    assert!(g.operation_enabled(KeyCode::Enter));
    let (x, y) = at(&buffer, "Parent:");
    assert_eq!(
        buffer[(x, y)].fg,
        Theme::new(level).style(Role::Muted).fg.unwrap_or_default()
    );
    assert!(
        buffer[(x, y)]
            .modifier
            .contains(ratatui::style::Modifier::DIM)
    );
    let (x, y) = at(&buffer, "資");
    assert!(
        !buffer[(x, y)]
            .modifier
            .contains(ratatui::style::Modifier::REVERSED)
    );
    at(&buffer, "▏");
    let button = g
        .operation_controls
        .iter()
        .find(|c| c.target == OperationTarget::Key(KeyCode::Enter))
        .unwrap();
    assert_eq!(
        buffer[(button.area.x, button.area.y)].modifier,
        Theme::new(level).style(Role::Cursor).add_modifier
    );
}

#[test]
fn f2_describes_the_next_action_and_keeps_hitboxes_and_labels_aligned() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    std::fs::write(&file, b"data").unwrap();
    for operation in [Operation::Copy, Operation::Move] {
        let mut app = dialog_fixture::dialog(
            Plan::paths(operation, vec![file.clone()], Some(root.path().into())).unwrap(),
        );
        if let Some(Dialog::Operation(d)) = &mut app.operations.dialog {
            d.text = "target".into();
        }
        let mut c = Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        );
        let mut positions = Vec::new();
        for enabled in [false, true, false] {
            let (buffer, g) = draw(&c.app, ColorLevel::NoColor);
            let button = g
                .operation_controls
                .iter()
                .find(|c| c.target == OperationTarget::Key(KeyCode::F(2)))
                .unwrap();
            assert_eq!(
                button.label,
                if enabled {
                    "F2 Disable SHA-256"
                } else {
                    "F2 Enable SHA-256"
                }
            );
            positions.push(button.area);
            assert_transfer_text(&buffer, enabled);
            c.handle(
                Event::Key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)),
                &g,
                std::time::Duration::ZERO,
            );
        }
        assert!(positions.iter().all(|p| *p == positions[0]));
        assert!(c.app.operations.outbox.is_empty());
    }
}

fn assert_transfer_text(buffer: &Buffer, enabled: bool) {
    let (from, _) = at(buffer, "From:");
    let (to, _) = at(buffer, "To:");
    assert_eq!(from, to);
    let (value, _) = at(buffer, "target▏");
    assert_eq!(value, from + 6);
    let text: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(text.contains(if enabled {
        "Verification: SHA-256 + metadata"
    } else {
        "Verification: Metadata"
    }));
    assert!(!text.contains("Approved destination") && !text.contains("F2 toggles"));
}
