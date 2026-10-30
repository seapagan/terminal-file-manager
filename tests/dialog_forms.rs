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
    for plan in [
        Plan::mkdir("/controlled".into()),
        Plan::create_file("/controlled".into()),
    ] {
        assert_create_form(plan);
    }
}
fn assert_create_form(plan: Plan) {
    let app = dialog_fixture::dialog(plan);
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

#[test]
fn invalid_create_renders_specific_feedback_and_recovers_after_editing() {
    let mut c = Controller::new(
        dialog_fixture::dialog(Plan::mkdir("/controlled".into())),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 40));
    for event in [
        Event::Paste("nested/directory".into()),
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    ] {
        c.handle(event, &g, std::time::Duration::ZERO);
    }
    let (buffer, disabled) = draw(&c.app, ColorLevel::NoColor);
    at(&buffer, "Enter one directory name");
    assert!(!disabled.operation_enabled(KeyCode::Enter));
    assert!(c.app.operations.outbox.is_empty());
    for event in [
        Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
        Event::Paste("資料".into()),
    ] {
        c.handle(event, &disabled, std::time::Duration::ZERO);
    }
    let (_, enabled) = draw(&c.app, ColorLevel::NoColor);
    assert!(enabled.operation_enabled(KeyCode::Enter));
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &enabled,
        std::time::Duration::ZERO,
    );
    assert!(c.app.operations.outbox.iter().any(
        |intent| matches!(intent, Intent::Submit(plan, _) if plan.destination.as_deref()
            == Some(std::path::Path::new("/controlled/資料")))
    ));
}
