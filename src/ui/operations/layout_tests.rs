use super::layout::Layout;
use crate::{
    app::App,
    color::Theme,
    operations::{Dialog, Operation, OperationDialog, Plan},
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

#[test]
fn scroll_cues_are_interior_and_all_geometry_stays_centred_after_resize() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("selected");
    std::fs::write(&file, b"kept").unwrap();
    let mut plan = Plan::paths(Operation::Copy, vec![file], Some(root.path().into())).unwrap();
    plan.operands = vec![plan.operands[0].clone(); 50];
    let mut app = App::new(root.path().into());
    app.operations.dialog = Some(Dialog::Operation(Box::new(OperationDialog {
        plan,
        pane: 0,
        base: root.path().into(),
        native_target: root.path().into(),
        text: "target".into(),
        incoming: String::new(),
        edited: false,
        field: 0,
        options: Default::default(),
        ready: true,
        preview_id: None,
        track_scope: false,
        retry_of: None,
        error: None,
    })));
    for width in [20, 40, 80, 120] {
        for height in [12, 13, 20, 30, 50] {
            let terminal = Rect::new(0, 0, width, height);
            let layout = Layout::new(&app, terminal);
            let top = layout.shell.outer.y;
            let bottom = height - 2 - layout.shell.outer.bottom();
            assert!(top.abs_diff(bottom) <= 1);
            if height >= 30 {
                assert!(layout.shell.outer.height <= height * 3 / 4);
            }
            assert!(layout.shell.cue.bottom() < layout.shell.outer.bottom());
            assert!(layout.shell.cue.y >= layout.body.bottom());
            assert_buffer(&app, &layout, width, height);
        }
    }
}

fn assert_buffer(app: &App, layout: &Layout, width: u16, height: u16) {
    let mut output = Terminal::new(TestBackend::new(width, height)).unwrap();
    output
        .draw(|f| {
            crate::ui::render(f, app, &Theme::new(colored_text::ColorLevel::NoColor));
        })
        .unwrap();
    let buffer = output.backend().buffer();
    if layout.body.height == 0 && layout.shell.cue.height > 0 {
        let cue: String = (layout.shell.cue.x..layout.shell.cue.right())
            .map(|x| buffer[(x, layout.shell.cue.y)].symbol())
            .collect();
        assert!(cue.contains("0/50") && cue.contains("Resize"));
    }
    assert_eq!(
        buffer[(layout.shell.outer.x, layout.shell.outer.bottom() - 1)].symbol(),
        "└"
    );
    assert_eq!(
        buffer[(
            layout.shell.outer.right() - 1,
            layout.shell.outer.bottom() - 1
        )]
            .symbol(),
        "┘"
    );
    for x in layout.shell.outer.x + 1..layout.shell.outer.right() - 1 {
        assert_eq!(buffer[(x, layout.shell.outer.bottom() - 1)].symbol(), "─");
    }
    for action in &layout.shell.actions {
        assert_eq!(buffer[(action.area.x, action.area.y)].symbol(), "[");
        assert_eq!(
            buffer[(action.area.right() - 1, action.area.y)].symbol(),
            "]"
        );
        assert_eq!(
            layout.shell.hit(action.area.x, action.area.y),
            Some(&action.action.intent)
        );
    }
}

#[test]
fn jobs_geometry_does_not_follow_selected_text_or_actions() {
    use crate::operations::JobState;
    let mut app = jobs_app();
    for (width, height) in [(40, 15), (80, 24), (120, 40), (300, 100)] {
        let area = Rect::new(0, 0, width, height);
        app.operations.cursor = 0;
        app.operations.jobs_evidence = false;
        let first = Layout::new(&app, area);
        app.operations.cursor = 1;
        app.operations.jobs_evidence = true;
        let second = Layout::new(&app, area);
        assert_eq!(first.shell.outer, second.shell.outer);
        assert_eq!(first.list, second.list);
        assert_eq!(first.body, second.body);
        assert!(first.shell.outer.width <= 120);
        assert!(first.shell.outer.height <= 28);
        app.operations.history[1].state = JobState::Running;
        let running = Layout::new(&app, area);
        assert_eq!(first.shell.outer, running.shell.outer);
        assert_eq!(first.body, running.body);
        assert_eq!(first.shell.footer, running.shell.footer);
        for state in [
            JobState::Failed,
            JobState::Partial,
            JobState::Unknown,
            JobState::Critical,
        ] {
            app.operations.history[1].state = state;
            app.operations.history[1].retry_eligible = true;
            for evidence in [false, true] {
                app.operations.jobs_evidence = evidence;
                assert_jobs_regions(&first, &Layout::new(&app, area));
            }
        }
        for button in &first.buttons {
            let next = running
                .buttons
                .iter()
                .find(|b| b.target == button.target)
                .unwrap();
            assert_eq!(button.area, next.area);
        }
    }
}

fn jobs_app() -> App {
    use crate::operations::{Job, JobState, Record};
    let mut app = App::new("/controlled".into());
    app.operations.show_history = true;
    for (id, path) in [
        (1, "/short".to_owned()),
        (2, format!("/{}", "long/".repeat(80))),
    ] {
        app.operations.history.push(Job {
            presentation: None,
            id,
            plan: Plan::mkdir(path.into()),
            options: Default::default(),
            state: JobState::Completed,
            record: Some(Record::default()),
            retry_eligible: false,
            progress: None,
            started: app.now,
            finished: Some(app.now),
            retry_of: None,
        });
    }
    app
}

fn assert_jobs_regions(first: &Layout, next: &Layout) {
    assert_eq!(first.shell.outer, next.shell.outer);
    assert_eq!(first.list, next.list);
    assert_eq!(first.body, next.body);
    assert_eq!(first.shell.footer, next.shell.footer);
    for key in [
        crossterm::event::KeyCode::Char('e'),
        crossterm::event::KeyCode::Esc,
    ] {
        assert_eq!(primary_area(first, key), primary_area(next, key));
    }
}

fn primary_area(layout: &Layout, key: crossterm::event::KeyCode) -> Rect {
    layout
        .buttons
        .iter()
        .find(|b| b.target == super::OperationTarget::Key(key))
        .unwrap()
        .area
}

#[test]
fn pending_preview_has_one_ascii_activity_cell_without_moving_controls() {
    for operation in [Operation::Trash, Operation::Delete] {
        let (_root, mut app) = pending_preview(operation);
        let layout = Layout::new(&app, Rect::new(0, 0, 80, 24));
        assert!(layout.header[0].text.starts_with("| Counting recursively"));
        assert_spinner_frames(&mut app, &layout);
        assert_spinner_stops(&mut app);
    }
}

fn pending_preview(operation: Operation) -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("kept");
    std::fs::write(&path, b"kept").unwrap();
    let mut app = App::new(root.path().into());
    app.operations.dialog = Some(Dialog::Operation(Box::new(OperationDialog {
        plan: Plan::paths(operation, vec![path], None).unwrap(),
        pane: 0,
        base: root.path().into(),
        native_target: root.path().into(),
        text: String::new(),
        incoming: String::new(),
        edited: false,
        field: 0,
        options: Default::default(),
        ready: false,
        preview_id: None,
        track_scope: false,
        retry_of: None,
        error: None,
    })));
    (root, app)
}

fn assert_spinner_frames(app: &mut App, layout: &Layout) {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| {
            crate::ui::render(f, app, &Theme::new(colored_text::ColorLevel::NoColor));
        })
        .unwrap();
    let mut previous = terminal.backend().buffer().clone();
    for (ms, glyph, changed) in [
        (0, '|', false),
        (149, '|', false),
        (150, '/', true),
        (299, '/', false),
        (300, '-', true),
        (450, '\\', true),
        (600, '|', true),
    ] {
        assert_eq!(
            super::animate_preview(app, std::time::Duration::from_millis(ms)),
            changed
        );
        terminal
            .draw(|f| {
                crate::ui::render(f, app, &Theme::new(colored_text::ColorLevel::NoColor));
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(
            previous
                .content
                .iter()
                .zip(&buffer.content)
                .filter(|(a, b)| a != b)
                .count(),
            usize::from(changed)
        );
        previous = buffer.clone();
        let next = Layout::new(app, Rect::new(0, 0, 80, 24));
        assert_eq!(next.header[0].text.chars().next(), Some(glyph));
        assert_eq!(next.shell.outer, layout.shell.outer);
        assert_eq!(next.body, layout.body);
        for (a, b) in next.buttons.iter().zip(&layout.buttons) {
            assert_eq!(a.area, b.area);
            assert_eq!(a.enabled, b.enabled);
        }
    }
}

fn assert_spinner_stops(app: &mut App) {
    app.help = true;
    assert!(!super::animate_preview(
        app,
        std::time::Duration::from_millis(750)
    ));
    app.help = false;
    if let Some(Dialog::Operation(d)) = &mut app.operations.dialog {
        d.error = Some("failed".into());
    }
    assert!(!super::animate_preview(
        app,
        std::time::Duration::from_millis(750)
    ));
    assert!(
        !Layout::new(app, Rect::new(0, 0, 80, 24)).header[0]
            .text
            .contains("Counting")
    );
    if let Some(Dialog::Operation(d)) = &mut app.operations.dialog {
        d.error = None;
        d.ready = true;
        let mut queue = crate::operations::Queue::new().unwrap();
        queue.preview(d.plan.clone()).unwrap();
        loop {
            if let crate::operations::Update::Preview { plan, .. } =
                queue.next(std::time::Duration::from_secs(5)).unwrap()
            {
                d.plan.preview = plan.preview;
                break;
            }
        }
    }
    assert!(!super::animate_preview(
        app,
        std::time::Duration::from_millis(750)
    ));
    assert!(
        !Layout::new(app, Rect::new(0, 0, 80, 24)).header[0]
            .text
            .contains("Counting")
    );
    app.operations.dialog = None;
    assert!(!super::animate_preview(
        app,
        std::time::Duration::from_millis(900)
    ));
}

#[test]
fn conflict_outer_rectangle_tracks_real_content_and_terminal_limit() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("a.txt");
    std::fs::write(&file, "owned").unwrap();
    let entry = crate::fs::inspect(&file);
    let (reply, _receiver) = std::sync::mpsc::sync_channel(1);
    let mut app = App::new(root.path().into());
    app.operations
        .conflicts
        .push_back(crate::operations::PendingConflict {
            id: 1,
            choice: 0,
            all: false,
            reply,
            event: crate::operations::Event::Conflict {
                source: Box::new(entry.clone()),
                destination: Box::new(entry),
                newer_available: true,
            },
        });
    for width in [80, 120, 160] {
        assert_conflict_widths(&mut app, width);
    }
    for width in [64, 65] {
        set_paths(&mut app, "source/a.txt", "destination/a.txt");
        let layout = Layout::new(&app, Rect::new(0, 0, width, 50));
        assert_eq!(
            layout
                .rows
                .iter()
                .any(|row| row.text.contains("Incoming (source)")
                    && row.text.contains("Existing (destination)")),
            width == 65
        );
    }
}

fn set_paths(app: &mut App, incoming: &str, existing: &str) {
    if let crate::operations::Event::Conflict {
        source,
        destination,
        ..
    } = &mut app.operations.conflicts.front_mut().unwrap().event
    {
        source.path = std::path::Path::new("/visual").join(incoming);
        destination.path = std::path::Path::new("/visual").join(existing);
    }
}

fn assert_conflict_widths(app: &mut App, width: u16) {
    let area = Rect::new(0, 0, width, 50);
    set_paths(app, "source/a.txt", "destination/a.txt");
    let short = Layout::new(app, area);
    assert!(short.shell.outer.width < 80);
    set_paths(
        app,
        "source/nested/incoming-folder/a.txt",
        "destination/nested/existing-folder/a.txt",
    );
    let long = Layout::new(app, area);
    assert!(long.shell.outer.width > short.shell.outer.width);
    assert!(long.shell.outer.x >= 1 && long.shell.outer.right() < width);
    assert!(
        long.shell
            .outer
            .x
            .abs_diff(width - long.shell.outer.right())
            <= 1
    );
    assert_eq!(long.rows.iter().any(|r| r.text.contains('…')), width == 80);
    set_paths(
        app,
        &format!("source/{}a.txt", "nested/".repeat(40)),
        &format!("destination/{}a.txt", "nested/".repeat(40)),
    );
    let exhausted = Layout::new(app, area);
    assert_eq!(exhausted.shell.outer.width, width - 2);
    assert!(
        exhausted
            .rows
            .iter()
            .any(|r| r.text.contains('…') && r.text.contains("a.txt"))
    );
}
