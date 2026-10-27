use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::Theme,
    operations::{Operation, Plan},
    ui::{self, OperationTarget},
};

mod dialog_fixture;
use dialog_fixture::dialog;

#[test]
fn action_rows_are_centred_as_groups_at_responsive_widths() {
    let app = dialog(Plan::mkdir("/controlled".into()));
    for width in [20, 40, 80, 120] {
        let geometry = ui::geometry(&app, Rect::new(0, 0, width, 30));
        let buttons: Vec<_> = geometry
            .operation_controls
            .iter()
            .filter(|control| matches!(control.target, OperationTarget::Key(_)))
            .collect();
        assert!(!buttons.is_empty());
        for y in buttons.iter().map(|control| control.area.y) {
            let row: Vec<_> = buttons
                .iter()
                .filter(|control| control.area.y == y)
                .collect();
            let left = row.first().unwrap().area.x;
            let right = width - row.last().unwrap().area.right();
            assert!(
                left.abs_diff(right) <= 1,
                "width {width}: margins {left}/{right}"
            );
        }
    }
}

#[test]
fn multi_source_preview_shows_one_root_and_unsplit_right_aligned_sizes() {
    let root = tempfile::Builder::new()
        .prefix("Ki-size-alignment-")
        .tempdir()
        .unwrap();
    let a = root.path().join("first.dat");
    let b = root.path().join("資料e\u{301}.dat");
    std::fs::write(&a, vec![0; 1024]).unwrap();
    std::fs::write(&b, vec![0; 10240]).unwrap();
    let app = dialog(Plan::paths(Operation::Trash, vec![a, b], None).unwrap());
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, &app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
    assert_eq!(
        text.matches(&ui::clean(&root.path().display().to_string()))
            .count(),
        1
    );
    let rows: Vec<String> = (0..40)
        .map(|y| (0..120).map(|x| buffer[(x, y)].symbol()).collect())
        .collect();
    let file_rows: Vec<_> = rows.iter().filter(|row| row.contains(".dat")).collect();
    assert_eq!(file_rows.len(), 2);
    assert!(file_rows.iter().all(|row| row.contains("KiB")));
    let units: Vec<_> = (0..40)
        .filter(|&y| rows[usize::from(y)].contains(".dat"))
        .filter_map(|y| {
            (0..118)
                .find(|&x| buffer[(x, y)].symbol() == "K" && buffer[(x + 1, y)].symbol() == "i")
                .map(|x| (x, y))
        })
        .collect();
    assert_eq!(units.len(), 2);
    assert_eq!(units[0].0, units[1].0);
}

#[test]
fn floating_metadata_owns_keys_without_mutating_the_underlying_pane() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use terminal_file_manager::input::{Bindings, Controller};
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let snapshot = terminal_file_manager::fs::list(
        root.path(),
        &terminal_file_manager::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    let mut app = App::new(root.path().into());
    app.complete_listing(0, 1, Ok(snapshot));
    app.popup = true;
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 80, 30));
    for code in [
        KeyCode::Char(' '),
        KeyCode::F(7),
        KeyCode::Backspace,
        KeyCode::Tab,
    ] {
        let requests = controller.handle(
            Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            &geometry,
            std::time::Duration::ZERO,
        );
        assert!(requests.is_empty());
        assert!(controller.app.popup);
        assert!(controller.app.operations.dialog.is_none());
        assert!(controller.app.panes[0].marks.is_empty());
        assert_eq!(controller.app.active, 0);
    }
}

#[test]
fn short_height_retains_a_visible_cancel_action_before_optional_controls() {
    let app = dialog(Plan::mkdir("/controlled".into()));
    let geometry = ui::geometry(&app, Rect::new(0, 0, 20, 5));
    assert!(
        geometry
            .operation_controls
            .iter()
            .any(|control| control.target == OperationTarget::Key(crossterm::event::KeyCode::Esc))
    );
}

#[test]
fn destructive_confirmation_uses_error_style_and_a_visible_safe_default() {
    use crossterm::event::KeyCode;
    use terminal_file_manager::color::Role;
    let app = delete_with_preview();
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let theme = Theme::new(level);
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        let geometry = ui::geometry(&app, Rect::new(0, 0, 80, 30));
        terminal
            .draw(|frame| {
                ui::render(frame, &app, &theme);
            })
            .unwrap();
        let at = |key| {
            geometry
                .operation_controls
                .iter()
                .find(|control| control.target == OperationTarget::Key(key))
                .unwrap()
                .area
        };
        let delete = at(KeyCode::Char('D'));
        assert_eq!(
            terminal.backend().buffer()[(delete.x, delete.y)].fg,
            theme.style(Role::Error).fg.unwrap_or_default()
        );
        assert_delete_warning(terminal.backend().buffer(), &theme);
        let cancel = at(KeyCode::Enter);
        assert_eq!(
            terminal.backend().buffer()[(cancel.x, cancel.y)].modifier,
            theme.style(Role::Cursor).add_modifier
        );
    }
}

fn delete_with_preview() -> App {
    use terminal_file_manager::operations::{Queue, Update};
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("selected");
    std::fs::write(&file, b"retained").unwrap();
    let plan = Plan::paths(Operation::Delete, vec![file], None).unwrap();
    let mut queue = Queue::new().unwrap();
    queue.preview(plan).unwrap();
    loop {
        if let Update::Preview { plan, .. } = queue.next(std::time::Duration::from_secs(5)).unwrap()
        {
            return dialog(*plan);
        }
    }
}

fn assert_delete_warning(buffer: &ratatui::buffer::Buffer, theme: &Theme) {
    use terminal_file_manager::color::Role;
    let warning_y = (0..30)
        .find(|&y| {
            let row: String = (0..80).map(|x| buffer[(x, y)].symbol()).collect();
            row.contains("PERMANENT DELETE cannot be undone.")
        })
        .unwrap();
    let warning_x = (0..80)
        .find(|&x| buffer[(x, warning_y)].symbol() == "P")
        .unwrap();
    assert_eq!(
        buffer[(warning_x, warning_y)].fg,
        theme.style(Role::Error).fg.unwrap_or_default()
    );
}

#[test]
fn successive_operation_metadata_resize_and_close_frames_restore_the_backdrop() {
    let mut app = dialog(Plan::mkdir("/controlled".into()));
    let theme = Theme::new(ColorLevel::NoColor);
    let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, &app, &theme);
        })
        .unwrap();
    app.operations.dialog = None;
    app.popup = true;
    terminal
        .draw(|frame| {
            ui::render(frame, &app, &theme);
        })
        .unwrap();
    terminal.backend_mut().resize(40, 20);
    terminal.resize(Rect::new(0, 0, 40, 20)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, &app, &theme);
        })
        .unwrap();
    app.popup = false;
    terminal
        .draw(|frame| {
            ui::render(frame, &app, &theme);
        })
        .unwrap();
    let mut fresh = Terminal::new(TestBackend::new(40, 20)).unwrap();
    fresh
        .draw(|frame| {
            ui::render(frame, &app, &theme);
        })
        .unwrap();
    assert_eq!(terminal.backend().buffer(), fresh.backend().buffer());
}

#[test]
fn conflict_keyboard_selection_focuses_the_action_enter_will_use() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use terminal_file_manager::input::{Bindings, Controller};
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(root.path().into());
    let receive = add_conflict(&mut app, root.path());
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    for (key, expected) in [
        (None, KeyCode::Char('s')),
        (Some(KeyCode::Right), KeyCode::Char('r')),
    ] {
        let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 80, 30));
        if let Some(key) = key {
            controller.handle(
                Event::Key(KeyEvent::new(key, KeyModifiers::NONE)),
                &geometry,
                std::time::Duration::ZERO,
            );
        }
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        terminal
            .draw(|frame| {
                ui::render(frame, &controller.app, &Theme::new(ColorLevel::NoColor));
            })
            .unwrap();
        let button = geometry
            .operation_controls
            .iter()
            .find(|control| control.target == OperationTarget::Key(expected))
            .unwrap();
        assert!(
            terminal.backend().buffer()[(button.area.x, button.area.y)]
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        );
        assert!(receive.try_recv().is_err());
    }
}

fn add_conflict(
    app: &mut App,
    path: &std::path::Path,
) -> std::sync::mpsc::Receiver<terminal_file_manager::operations::Decision> {
    use terminal_file_manager::operations::{Event as JobEvent, PendingConflict};
    let (reply, receive) = std::sync::mpsc::sync_channel(1);
    app.operations.conflicts.push_back(PendingConflict {
        id: 1,
        choice: 0,
        all: false,
        reply,
        event: JobEvent::Conflict {
            source: Box::new(terminal_file_manager::fs::inspect(path)),
            destination: Box::new(terminal_file_manager::fs::inspect(path)),
            newer_available: true,
        },
    });
    receive
}
