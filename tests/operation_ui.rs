use colored_text::ColorLevel;
use crossterm::event::KeyCode;
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::Theme,
    operations::{Dialog, Event, Job, JobState, OperationDialog, PendingConflict, Plan, Record},
    ui,
};

fn mkdir() -> App {
    let mut app = App::new("/controlled".into());
    app.operations.dialog = Some(Dialog::Operation(Box::new(OperationDialog {
        plan: Plan::mkdir("/controlled".into()),
        pane: 0,
        base: "/controlled".into(),
        native_target: "/controlled".into(),
        text: "資料\u{1b}[31m".into(),
        incoming: String::new(),
        edited: false,
        field: 0,
        options: Default::default(),
        ready: true,
        error: None,
        preview_id: None,
        track_scope: false,
        retry_of: None,
    })));
    app
}

fn draw(app: &App, width: u16, height: u16, level: ColorLevel) -> (String, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let geometry = terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(level));
        })
        .unwrap();
    let text = geometry
        .buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    (text, ui::geometry(app, Rect::new(0, 0, width, height)))
}

#[test]
fn short_operation_uses_content_sized_modal_and_specific_field() {
    let (text, geometry) = draw(&mkdir(), 120, 40, ColorLevel::NoColor);
    assert!(text.contains("Create directory"), "{text}");
    assert!(text.contains("Directory name:"), "{text}");
    assert!(
        geometry
            .operation_controls
            .iter()
            .all(|control| control.area.y < 30)
    );
    assert!(!text.contains("Path/name/mode"));
    assert!(!text.contains("F2 SHA-256"));
}

#[test]
fn narrow_controls_wrap_without_losing_cancel_or_run() {
    let app = mkdir();
    let (_, geometry) = draw(&app, 24, 20, ColorLevel::NoColor);
    assert!(
        geometry
            .operation_controls
            .iter()
            .any(|control| control.target == ui::OperationTarget::Key(KeyCode::Enter))
    );
    assert!(
        geometry
            .operation_controls
            .iter()
            .any(|control| control.target == ui::OperationTarget::Key(KeyCode::Esc))
    );
    assert!(
        geometry
            .operation_controls
            .iter()
            .all(|control| control.area.right() <= 24)
    );
}

#[test]
fn unicode_and_controls_render_safely_at_every_size_and_color() {
    let app = mkdir();
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        for width in [1, 20, 40, 120] {
            let (text, _) = draw(&app, width, 25, level);
            assert!(!text.contains('\u{1b}'));
            if width == 120 {
                assert!(text.contains("\\u{1b}[31m"));
            }
        }
    }
}

#[test]
fn completed_jobs_show_readable_details_and_no_active_controls() {
    let mut app = App::new("/controlled".into());
    app.operations.show_history = true;
    app.operations.history.push(Job {
        retry_eligible: true,
        id: 7,
        plan: Plan::mkdir("/controlled/new".into()),
        options: Default::default(),
        state: JobState::Completed,
        record: Some(Record::default()),
        progress: None,
        started: app.now,
        finished: Some(app.now),
        retry_of: None,
    });
    let (text, geometry) = draw(&app, 120, 40, ColorLevel::NoColor);
    assert!(text.contains("Create directory"), "{text}");
    assert!(!text.contains("Job 7 Mkdir"));
    assert!(!text.contains("None"));
    assert!(text.contains("session only"));
    assert!(
        geometry
            .operation_controls
            .iter()
            .all(|control| !control.label.starts_with("C "))
    );
}

#[test]
fn conflict_disables_unsafe_choices_and_formats_optional_metadata() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("file");
    let destination = root.path().join("directory");
    std::fs::write(&source, "data").unwrap();
    std::fs::create_dir(&destination).unwrap();
    let mut app = App::new(root.path().to_owned());
    let (reply, _) = std::sync::mpsc::sync_channel(1);
    app.operations.conflicts.push_back(PendingConflict {
        id: 3,
        choice: 0,
        all: false,
        reply,
        event: Event::Conflict {
            source: Box::new(terminal_file_manager::fs::inspect(&source)),
            destination: Box::new(terminal_file_manager::fs::inspect(&destination)),
            newer_available: false,
        },
    });
    let (text, geometry) = draw(&app, 120, 40, ColorLevel::NoColor);
    assert!(text.contains("Replace disabled"));
    assert!(text.contains("Newer disabled"));
    assert!(!text.contains("Some("));
    assert!(!text.contains("SystemTime"));
    for key in ['r', 'n'] {
        let control = geometry
            .operation_controls
            .iter()
            .find(|control| control.target == ui::OperationTarget::Key(KeyCode::Char(key)))
            .unwrap();
        assert!(!control.enabled);
        assert_eq!(
            geometry.operation_target(control.area.x, control.area.y),
            None
        );
    }
}

fn completed_job(app: &App, id: u64) -> Job {
    Job {
        retry_eligible: true,
        id,
        plan: Plan::mkdir(format!("/controlled/{id}").into()),
        options: Default::default(),
        state: JobState::Completed,
        record: Some(Record::default()),
        progress: None,
        started: app.now,
        finished: Some(app.now),
        retry_of: None,
    }
}

#[test]
fn selected_job_stays_in_a_bounded_clickable_list_after_resize() {
    let mut app = App::new("/controlled".into());
    app.operations.show_history = true;
    app.operations.history = (0..20).map(|id| completed_job(&app, id)).collect();
    app.operations.cursor = 14;
    for width in [20, 40, 120] {
        let (_, geometry) = draw(&app, width, 25, ColorLevel::NoColor);
        let jobs: Vec<_> = geometry
            .operation_controls
            .iter()
            .filter_map(|control| {
                if let ui::OperationTarget::Job(index) = control.target {
                    Some(index)
                } else {
                    None
                }
            })
            .collect();
        assert!(jobs.contains(&14));
        assert!(jobs.len() <= 5);
        assert!(
            geometry
                .operation_controls
                .iter()
                .all(|control| control.area.bottom() <= 23)
        );
    }
}

#[test]
fn every_button_hit_rectangle_matches_its_complete_visible_label() {
    let app = mkdir();
    for width in [20, 24, 40, 120] {
        let mut terminal = Terminal::new(TestBackend::new(width, 25)).unwrap();
        terminal
            .draw(|frame| {
                ui::render(frame, &app, &Theme::new(ColorLevel::NoColor));
            })
            .unwrap();
        let geometry = ui::geometry(&app, Rect::new(0, 0, width, 25));
        for control in geometry.operation_controls {
            if let ui::OperationTarget::Key(_) = control.target {
                let visible: String = (control.area.x..control.area.right())
                    .map(|x| terminal.backend().buffer()[(x, control.area.y)].symbol())
                    .collect();
                assert_eq!(visible, format!("[{}]", control.label));
            }
        }
    }
}

#[test]
fn mouse_selects_the_visible_field_without_running_it() {
    use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use std::time::Duration;
    use terminal_file_manager::input::{Bindings, Controller};
    let mut app = mkdir();
    if let Some(Dialog::Operation(dialog)) = &mut app.operations.dialog {
        dialog.field = 1;
    }
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 40, 25));
    let field = geometry
        .operation_controls
        .iter()
        .find(|control| control.target == ui::OperationTarget::Field(0))
        .unwrap();
    controller.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: field.area.x,
            row: field.area.y,
            modifiers: KeyModifiers::NONE,
        }),
        &geometry,
        Duration::ZERO,
    );
    assert!(
        matches!(&controller.app.operations.dialog, Some(Dialog::Operation(dialog)) if dialog.field == 0)
    );
    assert!(controller.app.operations.outbox.is_empty());
}

#[test]
fn notices_have_textual_severity_expire_and_leave_the_footer_visible() {
    use std::time::Duration;
    use terminal_file_manager::operations::NoticeSeverity;
    let mut app = App::new("/controlled".into());
    app.operations
        .notify("Job 1 completed".into(), app.now, NoticeSeverity::Success);
    let (text, _) = draw(&app, 120, 25, ColorLevel::NoColor);
    assert!(text.contains("Success: Job 1 completed — F9 details"));
    assert!(text.contains("F8 Trash"));
    app.now += Duration::from_secs(6);
    let (text, _) = draw(&app, 120, 25, ColorLevel::NoColor);
    assert!(!text.contains("Success: Job 1 completed"));
}

#[test]
fn tiny_terminal_sizes_never_produce_outside_hit_rectangles() {
    let app = mkdir();
    for width in [0, 1, 2, 20] {
        for height in [0, 1, 2, 5] {
            let (_, geometry) = draw(&app, width, height, ColorLevel::NoColor);
            assert!(geometry.operation_controls.iter().all(|control| control.area.right() <= width && control.area.bottom() <= height));
        }
    }
}

#[test]
fn mouse_and_keyboard_select_jobs_while_detail_scroll_stays_bounded() {
    use crossterm::event::{
        Event, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::time::Duration;
    use terminal_file_manager::input::{Bindings, Controller};
    let mut app = App::new("/controlled".into());
    app.operations.history = (0..8).map(|id| completed_job(&app, id)).collect();
    app.operations.show_history = true;
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 40, 25));
    let job = geometry
        .operation_controls
        .iter()
        .find(|control| control.target == ui::OperationTarget::Job(2))
        .unwrap();
    controller.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: job.area.x,
            row: job.area.y,
            modifiers: KeyModifiers::NONE,
        }),
        &geometry,
        Duration::ZERO,
    );
    assert_eq!(controller.app.operations.cursor, 2);
    controller.handle(
        Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        &geometry,
        Duration::ZERO,
    );
    assert_eq!(controller.app.operations.cursor, 3);
    controller.app.operations.scroll = usize::MAX;
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 20, 10));
    controller.handle(
        Event::Key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
        &geometry,
        Duration::ZERO,
    );
    assert!(controller.app.operations.scroll <= geometry.operation_scroll_limit);
    assert!(controller.app.operations.outbox.is_empty());
}

#[test]
fn preview_failure_keeps_its_reason_and_never_claims_counting_or_success() {
    let mut app = mkdir();
    if let Some(Dialog::Operation(dialog)) = &mut app.operations.dialog {
        dialog.plan.operation = terminal_file_manager::operations::Operation::Trash;
        dialog.ready = false;
        dialog.error = Some("Cannot inspect selected path: permission denied".into());
    }
    let (text, geometry) = draw(&app, 120, 30, ColorLevel::NoColor);
    assert!(text.contains("Preview unavailable"));
    let unframed: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '│' | '─'))
        .collect();
    assert!(unframed.contains("Cannotinspectselectedpath:permissiondenied"));
    assert!(!text.contains("Counting recursively"));
    assert!(!text.contains("source left"));
    assert!(
        geometry
            .operation_controls
            .iter()
            .filter(
                |control| control.enabled && matches!(control.target, ui::OperationTarget::Key(_))
            )
            .all(|control| control.target == ui::OperationTarget::Key(KeyCode::Enter))
    );
}

#[test]
fn failed_request_evidence_survives_transient_expiry_without_a_job() {
    use std::time::Duration;
    let mut app = App::new("/controlled".into());
    app.operations.fail(
        "Destination could not be inspected: access denied".into(),
        app.now,
    );
    app.now += Duration::from_secs(6);
    app.operations.show_history = true;
    let (text, _) = draw(&app, 120, 30, ColorLevel::NoColor);
    assert!(text.contains("Latest request not started"));
    assert!(text.contains("access denied"));
    assert!(text.contains("No jobs this session"));
    assert!(!text.contains("source left"));
}

#[test]
fn critical_banner_outlives_notices_without_covering_the_footer_or_taking_focus() {
    use std::time::Duration;
    use terminal_file_manager::{
        fs::EntryKind,
        operations::{EntryResult, Outcome},
    };
    let mut app = mkdir();
    let mut job = completed_job(&app, 1);
    job.state = JobState::Critical;
    job.record = Some(Record {
        entries: vec![EntryResult {
            trash: None,
            recovery: None,
            source: "/controlled/item".into(),
            destination: None,
            kind: EntryKind::File,
            outcome: Outcome::SafetyIncident,
            bytes: 0,
            checksum_verified: false,
            created_directory: false,
            detail: "OS reported permanent deletion during Trash".into(),
        }],
    });
    app.operations.history.push(job);
    app.now += Duration::from_secs(600);
    let (text, _) = draw(&app, 120, 30, ColorLevel::NoColor);
    assert!(text.contains("CRITICAL · F9 details"));
    assert!(text.contains("F8 Trash"));
    assert!(matches!(&app.operations.dialog, Some(Dialog::Operation(dialog)) if dialog.field == 0));
}
