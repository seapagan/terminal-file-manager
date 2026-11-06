use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, SystemTime};
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Bindings, Controller},
    notifications::{Notifications, Severity},
    ui,
};

#[path = "notification_ux/destructive_success.rs"]
mod destructive_success;
#[path = "notification_ux/final_corrections.rs"]
mod final_corrections;
#[path = "notification_ux/polish.rs"]
mod polish;
#[path = "notification_ux/redesign.rs"]
mod redesign;
#[path = "notification_ux/review_regressions.rs"]
mod review_regressions;

fn controller() -> Controller {
    Controller::new(
        App::new("owned-notifications".into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}

fn draw(c: &Controller, width: u16) -> (ratatui::buffer::Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
    let mut geometry = ui::geometry(&c.app, terminal.size().unwrap().into());
    terminal
        .draw(|frame| {
            geometry = ui::render(
                frame,
                &c.app,
                &Theme::new(colored_text::ColorLevel::NoColor),
            )
        })
        .unwrap();
    (terminal.backend().buffer().clone(), geometry)
}

fn text(buffer: &ratatui::buffer::Buffer, area: ratatui::layout::Rect) -> String {
    (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn independent_job_failures_share_a_toast_but_keep_each_history_link() {
    let mut notices = Notifications::default();
    for id in [7, 8] {
        notices.record(
            "Permission denied (os error 13)".into(),
            SystemTime::UNIX_EPOCH,
            Severity::Error,
            Some(id),
        );
    }
    assert_eq!(notices.visible(), [1]);
    assert_eq!(notices.history.len(), 2);
    assert_eq!(
        notices
            .history
            .iter()
            .map(|event| event.job)
            .collect::<Vec<_>>(),
        [Some(7), Some(8)]
    );
    notices.record(
        "A different failure".into(),
        SystemTime::UNIX_EPOCH,
        Severity::Error,
        Some(9),
    );
    assert_eq!(notices.visible(), [2, 1]);
    notices.advance(Duration::from_secs(3));
    notices.record(
        "Permission denied (os error 13)".into(),
        SystemTime::UNIX_EPOCH,
        Severity::Error,
        Some(10),
    );
    assert_eq!(notices.visible(), [3, 2, 1]);
}

#[test]
fn grouped_toast_count_survives_queue_pressure_and_remains_visible_at_narrow_widths() {
    let mut c = controller();
    c.app
        .notifications
        .notify("Unrelated warning".into(), c.app.now, Severity::Warning);
    for id in 1..=40 {
        c.app.notifications.record(
            "Permission denied (os error 13)".into(),
            c.app.now,
            Severity::Error,
            Some(id),
        );
    }
    assert_eq!(c.app.notifications.toast_occurrences(40), 40);
    assert_eq!(c.app.notifications.history.len(), 41);
    for width in [20, 40, 80, 120] {
        let (buffer, geometry) = draw(&c, width);
        assert_eq!(c.app.notifications.visible(), [40, 0]);
        assert!(text(&buffer, geometry.toasts[0].area).contains("×40"));
    }
}

#[test]
fn a_group_keeps_its_count_until_the_latest_toast_expires() {
    let mut notices = Notifications::default();
    notices.record(
        "failure".into(),
        SystemTime::UNIX_EPOCH,
        Severity::Error,
        Some(1),
    );
    notices.advance(Duration::from_secs(1));
    notices.record(
        "failure".into(),
        SystemTime::UNIX_EPOCH,
        Severity::Error,
        Some(2),
    );
    notices.advance(Duration::from_secs(5));
    assert_eq!(notices.visible(), [1]);
    assert_eq!(notices.toast_occurrences(1), 2);
    notices.advance(Duration::from_secs(6));
    assert!(notices.visible().is_empty());
    assert_eq!(notices.history.len(), 2);
}

#[test]
fn toast_heading_message_and_hint_have_separate_word_wrapped_rows() {
    let mut c = controller();
    c.app
        .notify(Severity::Error, "Permission denied (os error 13)");
    let (buffer, geometry) = draw(&c, 80);
    let toast = &geometry.toasts[0];
    let content = text(&buffer, toast.area);
    assert!(
        content
            .lines()
            .any(|line| line.contains("Permission denied")),
        "{content}"
    );
    assert!(!content.contains("os error"), "{content}");
    assert!(
        content.lines().any(|line| line.contains("Alt+h details")),
        "{content}"
    );
    assert_eq!(toast.area.bottom(), geometry.footer.y);
}

#[test]
fn history_is_a_compact_selectable_list_with_independent_details() {
    let mut c = controller();
    for summary in ["First event", "Second event", "Third event"] {
        c.app.notify(Severity::Warning, summary);
    }
    c.app.notifications.open = true;
    c.app.notifications.cursor = 1;
    let (buffer, geometry) = draw(&c, 100);
    let layout = geometry.notification_history.as_ref().unwrap();
    let content = text(&buffer, layout.shell.outer);
    for summary in ["First event", "Second event", "Third event"] {
        assert!(content.contains(summary), "{content}");
    }
    assert!(layout.shell.outer.height <= 10);
    assert!(content.contains("> Warning"), "{content}");
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        &geometry,
        Duration::ZERO,
    );
    assert_eq!(c.app.notifications.cursor, 2);
}

fn complete(c: &mut Controller, plan: terminal_file_manager::operations::Plan) {
    use terminal_file_manager::operations::{Decision, Update, execute};
    let id = c.app.operations.history.len() as u64 + 1;
    let record = execute(&plan, Default::default(), &false.into(), |_| Decision::Skip);
    c.register_operation(id, plan, Default::default(), None);
    c.complete_operation(Update::Finished { id, record });
}

#[test]
fn copy_and_move_to_self_use_concise_copy_without_altering_source_or_diagnostics() {
    use terminal_file_manager::operations::{Operation, Plan};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("owned");
    std::fs::write(&path, "preserved").unwrap();
    let mut c = controller();
    for operation in [Operation::Copy, Operation::Move] {
        complete(
            &mut c,
            Plan::paths(operation, vec![path.clone()], Some(root.path().into())).unwrap(),
        );
        let event = c.app.notifications.history.last().unwrap();
        assert!(event.presentation.title.ends_with("failed"));
        assert_eq!(
            event.presentation.message,
            format!(
                "Cannot {} “owned” onto itself.",
                if operation == Operation::Copy {
                    "copy"
                } else {
                    "move"
                }
            )
        );
        assert!(event.text.contains("Source and destination"));
        assert_eq!(std::fs::read(&path).unwrap(), b"preserved");
    }
    assert_eq!(c.app.notifications.visible().len(), 2);
}

#[test]
fn collisions_on_different_targets_and_successes_keep_distinct_operation_context() {
    use terminal_file_manager::operations::Plan;
    let root = tempfile::tempdir().unwrap();
    let mut c = controller();
    for parent in ["a", "b"] {
        let directory = root.path().join(parent);
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("same-name");
        std::fs::create_dir(&path).unwrap();
        complete(&mut c, Plan::mkdir(path.clone()));
        assert_eq!(
            c.app
                .notifications
                .history
                .last()
                .unwrap()
                .presentation
                .message,
            "“same-name” already exists. Choose a different name."
        );
        assert!(path.is_dir());
    }
    assert_eq!(c.app.notifications.visible().len(), 2);
    complete(&mut c, Plan::create_file(root.path().join("created")));
    let event = c.app.notifications.history.last().unwrap();
    assert_eq!(event.presentation.title, "Create file complete");
    assert_eq!(event.presentation.message, "“created”");
    assert_eq!(std::fs::read(root.path().join("created")).unwrap(), b"");
}

#[test]
fn permission_failure_keeps_raw_native_diagnostics_only_in_history_and_jobs() {
    use terminal_file_manager::operations::{EntryResult, Outcome, Plan, Record, Update};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("denied");
    let mut c = controller();
    c.register_operation(1, Plan::mkdir(path.clone()), Default::default(), None);
    let detail = "Permission denied (os error 13)";
    c.complete_operation(Update::Finished {
        id: 1,
        record: Record {
            entries: vec![EntryResult {
                source: path.clone(),
                destination: Some(path),
                kind: terminal_file_manager::fs::EntryKind::Directory,
                outcome: Outcome::Failed,
                bytes: 0,
                checksum_verified: false,
                created_directory: false,
                detail: detail.into(),
                trash: None,
                recovery: None,
            }],
        },
    });
    let event = &c.app.notifications.history[0];
    assert_eq!(
        event.presentation.message,
        "Permission denied for “denied”."
    );
    assert!(event.text.contains(detail));
    assert_eq!(
        c.app.operations.history[0].record.as_ref().unwrap().entries[0].detail,
        detail
    );
    let (buffer, geometry) = draw(&c, 80);
    let toast = text(&buffer, geometry.toasts[0].area);
    assert!(toast.contains("Permission denied for “denied”."));
    assert!(!toast.contains("os error"));
}

#[test]
fn toast_surface_accent_heading_and_hint_respect_colour_capabilities() {
    use colored_text::ColorLevel::{Ansi16, Ansi256, NoColor, TrueColor};
    use ratatui::style::{Color, Modifier};
    use terminal_file_manager::color::Role;
    let mut c = controller();
    for severity in [
        Severity::Info,
        Severity::Success,
        Severity::Warning,
        Severity::Error,
    ] {
        c.app.notifications.dismiss_toasts();
        c.app.notify(severity, "A readable message");
        for level in [NoColor, Ansi16, Ansi256, TrueColor] {
            let theme = Theme::new(level);
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            let mut geometry = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 80, 24));
            terminal
                .draw(|frame| geometry = ui::render(frame, &c.app, &theme))
                .unwrap();
            let area = geometry.toasts[0].area;
            let buffer = terminal.backend().buffer();
            let accent = &buffer[(area.x, area.y + 1)];
            let heading = &buffer[(area.x + 2, area.y + 1)];
            let message = &buffer[(area.x + 2, area.y + 2)];
            let hint = &buffer[(area.x + 2, area.bottom() - 2)];
            assert_eq!(accent.symbol(), "▌");
            assert_eq!(
                accent.fg,
                theme.style(severity.role()).fg.unwrap_or(Color::Reset)
            );
            assert!(heading.modifier.contains(Modifier::BOLD));
            assert!(!message.modifier.contains(Modifier::BOLD));
            assert!(hint.modifier.contains(Modifier::ITALIC));
            if level == NoColor {
                assert_eq!((message.fg, message.bg), (Color::Reset, Color::Reset));
            } else {
                assert_ne!(message.bg, theme.style(Role::Cursor).bg.unwrap());
            }
        }
    }
}

#[test]
fn history_pages_mouse_selection_resize_and_details_preserve_each_event() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let mut c = controller();
    for index in 0..25 {
        c.app.notify(
            Severity::Info,
            format!("Event {index}: {}", "diagnostic ".repeat(200)),
        );
    }
    c.app.notifications.open = true;
    c.app.notifications.cursor = 24;
    let (_, geometry) = draw(&c, 100);
    press(&mut c, &geometry, KeyCode::PageUp);
    assert_eq!(c.app.notifications.cursor, 16);
    let (_, geometry) = draw(&c, 100);
    let layout = geometry.notification_history.as_ref().unwrap();
    let index = layout.start;
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: layout.shell.body.x,
            row: layout.shell.body.y,
            modifiers: KeyModifiers::NONE,
        }),
        &geometry,
        Duration::ZERO,
    );
    assert_eq!(c.app.notifications.cursor, index);
    press(&mut c, &geometry, KeyCode::Char('e'));
    let (_, geometry) = draw(&c, 100);
    press(&mut c, &geometry, KeyCode::PageDown);
    assert!(c.app.notifications.scroll > 0);
    for width in [20, 40, 80, 120] {
        c.handle(Event::Resize(width, 24), &geometry, Duration::ZERO);
        let (_, geometry) = draw(&c, width);
        let layout = geometry.notification_history.unwrap();
        assert!(layout.shell.outer.right() <= width);
        assert!(c.app.notifications.scroll <= layout.limit);
        assert_eq!(c.app.notifications.cursor, index);
    }
    assert_eq!(c.app.notifications.history.len(), 25);
}

fn press(c: &mut Controller, geometry: &ui::Geometry, code: KeyCode) {
    c.handle(
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        geometry,
        Duration::ZERO,
    );
}
