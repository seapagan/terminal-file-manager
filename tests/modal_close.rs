//! Title close controls use the same safe Esc route as their owning layer.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Bindings, Controller, chord},
    operations::{Dialog, Intent, Operation, Plan},
    ui::{self, modal},
};

fn controller(app: App) -> Controller {
    Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}

fn key(c: &mut Controller, text: &str) {
    let (code, modifiers) = chord(text).unwrap();
    send(c, Event::Key(KeyEvent::new(code, modifiers)));
}

fn send(c: &mut Controller, event: Event) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
    assert!(c.handle(event, &g, Duration::ZERO).is_empty());
}

fn mouse(c: &mut Controller, area: Rect, button: MouseButton) {
    send(
        c,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(button),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }),
    );
}

fn close(c: &mut Controller) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
    mouse(c, g.modal_close.expect("visible close"), MouseButton::Left);
}

#[test]
fn close_header_has_exact_cells_and_preserves_shell_geometry() {
    for width in [1, 4, 12, 20, 40, 80, 120] {
        for level in [
            ColorLevel::NoColor,
            ColorLevel::Ansi16,
            ColorLevel::Ansi256,
            ColorLevel::TrueColor,
        ] {
            for title in ["Copy", "界e\u{301} title"] {
                check_header(width, level, title);
            }
        }
    }
}

fn check_header(width: u16, level: ColorLevel, title: &str) {
    let spec = modal::Spec::<()> {
        title,
        help: true,
        content_width: 50,
        pinned_rows: 0,
        footer_rows: 0,
        actions: &[],
    };
    let layout = spec.measure(Rect::new(0, 0, width, 20), |_| 2);
    let mut terminal = Terminal::new(TestBackend::new(width, 20)).unwrap();
    terminal
        .draw(|f| modal::render_measured(f, &layout, title, &Theme::new(level), Role::Header))
        .unwrap();
    let b = terminal.backend().buffer();
    if let Some(close) = layout.close {
        assert_eq!(shown(b, close), "[×]");
        assert_eq!(close.right(), layout.outer.right() - 1);
        assert!(close.x > layout.outer.x + 2 + modal::width(title));
        if let Some(help) = layout.help {
            assert_eq!(shown(b, help), "[?]");
            assert_eq!(close.x, help.right() + 1);
            assert!(!help.intersects(close));
        }
    } else {
        assert!(layout.outer.width < modal::width(title) + 7);
    }
    let without_help = modal::Spec {
        help: false,
        ..spec
    }
    .measure(Rect::new(0, 0, width, 20), |_| 2);
    // Title controls never alter the measured content or action bands.
    assert_eq!(layout.outer, without_help.outer);
    assert_eq!(layout.body, without_help.body);
    assert!(without_help.help.is_none());
}

fn shown(b: &ratatui::buffer::Buffer, area: Rect) -> String {
    (area.x..area.right())
        .map(|x| b[(x, area.y)].symbol())
        .collect()
}

#[test]
fn closing_an_operation_cancels_its_preview_and_never_submits() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preserved");
    std::fs::write(&path, b"owned").unwrap();
    for operation in [
        Operation::Copy,
        Operation::Move,
        Operation::Rename,
        Operation::Trash,
        Operation::Delete,
        Operation::ReadOnly(true),
    ] {
        let plan = Plan::paths(
            operation,
            vec![path.clone()],
            Some(root.path().join("destination")),
        )
        .unwrap();
        let mut app = dialog_fixture::dialog(plan.clone());
        let Some(Dialog::Operation(dialog)) = &mut app.operations.dialog else {
            unreachable!()
        };
        dialog.preview_id = Some(77);
        app.operations.outbox.push(Intent::Preview(plan));
        let mut c = controller(app);
        close(&mut c);
        assert!(c.app.operations.dialog.is_none());
        assert!(matches!(
            c.app.operations.outbox.as_slice(),
            [Intent::Cancel(77)]
        ));
        assert!(!c.quit);
    }
    assert_eq!(std::fs::read(path).unwrap(), b"owned");
}

#[test]
fn create_close_and_adjacent_or_nonleft_clicks_keep_input_isolated() {
    let root = tempfile::tempdir().unwrap();
    for plan in [
        Plan::create_file(root.path().into()),
        Plan::mkdir(root.path().into()),
    ] {
        let mut c = controller(dialog_fixture::dialog(plan));
        key(&mut c, "n");
        let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
        let area = g.modal_close.unwrap();
        mouse(&mut c, area, MouseButton::Right);
        mouse(
            &mut c,
            Rect::new(area.x - 1, area.y, 1, 1),
            MouseButton::Left,
        );
        assert!(matches!(&c.app.operations.dialog, Some(Dialog::Operation(d)) if d.text == "n"));
        close(&mut c);
        assert!(c.app.operations.dialog.is_none());
        assert!(c.app.operations.outbox.is_empty());
        assert!(!root.path().join("n").exists());
    }
}

#[test]
fn close_dismisses_only_context_help_then_restores_and_cancels_the_parent() {
    let mut c = controller(App::new("owned-close-fixture".into()));
    key(&mut c, "Alt+b");
    key(&mut c, "?");
    let parent = c.app.pattern.as_ref().unwrap();
    let saved = (parent.expression().to_owned(), parent.focus, parent.scroll);
    let before = ui::geometry(&c.app, Rect::new(0, 0, 120, 32))
        .pattern
        .unwrap()
        .shell;
    key(&mut c, "F1");
    close(&mut c);
    assert!(c.app.context_help.is_none());
    let after = c.app.pattern.as_ref().unwrap();
    assert_eq!(after.expression(), saved.0);
    assert_eq!(after.focus, saved.1);
    assert_eq!(after.scroll, saved.2);
    let restored = ui::geometry(&c.app, Rect::new(0, 0, 120, 32))
        .pattern
        .unwrap()
        .shell;
    assert_eq!(restored.outer, before.outer);
    assert_eq!(restored.body, before.body);
    close(&mut c);
    assert!(c.app.pattern.is_none());
    assert!(c.app.operations.outbox.is_empty());
}

#[test]
fn help_picker_close_preserves_document_and_does_not_overlap_jump_control() {
    let mut c = controller(App::new("owned-close-fixture".into()));
    key(&mut c, "?");
    key(&mut c, "PageDown");
    let scroll = c.app.overlay_scroll;
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
    assert!(
        !g.modal_close
            .unwrap()
            .intersects(g.help.unwrap().search.unwrap())
    );
    key(&mut c, "/");
    close(&mut c);
    assert!(c.app.help && c.app.help_picker.is_none());
    assert_eq!(c.app.overlay_scroll, scroll);
    close(&mut c);
    assert!(!c.app.help && !c.quit);
}

#[test]
fn jobs_metadata_and_exit_close_follow_their_existing_escape_behaviour() {
    let mut c = controller(App::new("owned-close-fixture".into()));
    c.app.operations.show_history = true;
    close(&mut c);
    assert!(!c.app.operations.show_history);
    key(&mut c, "Alt+Enter");
    close(&mut c);
    assert!(!c.app.popup);
    for dialog in [
        Dialog::Exit { save: true },
        Dialog::IncidentExit {
            save: true,
            typed: "ACKNOWLEDGE".into(),
        },
    ] {
        c.app.operations.dialog = Some(dialog);
        close(&mut c);
        assert!(c.app.operations.dialog.is_none());
        assert!(!c.quit && !c.save_session);
        assert!(c.app.operations.outbox.is_empty());
    }
}

#[test]
fn conflict_close_sends_only_the_existing_cancel_decision() {
    let (_root, mut app, response) = conflict_fixture::fixture();
    conflict_fixture::edit(&mut app, |_, _, newer| *newer = false);
    let (_, _, text) = conflict_fixture::draw(&app, 120, 32, ColorLevel::NoColor);
    assert!(text.contains("[?] [×]"));
    let mut c = controller(app);
    close(&mut c);
    assert!(matches!(
        response.try_recv().unwrap(),
        terminal_file_manager::operations::Decision::Cancel
    ));
    assert!(c.app.operations.outbox.is_empty());
    assert!(!c.quit);
}

mod conflict_fixture;
mod dialog_fixture;
