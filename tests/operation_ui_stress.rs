use colored_text::ColorLevel;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::{
    fs,
    time::{Duration, Instant, SystemTime},
};
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs::EntryKind,
    input::{Bindings, Controller},
    operations::{EntryResult, Job, JobState, Operation, Outcome, Plan, Record},
    ui,
};

fn fixture() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&target).unwrap();
    let plan = Plan::paths(Operation::Copy, vec![source.clone()], Some(target.clone())).unwrap();
    let long = "nested_segment_0123456789/".repeat(40);
    let mut entries: Vec<_> = (0..2500)
        .map(|index| EntryResult {
            source: source.join(format!("{long}file-{index}")),
            destination: Some(target.join(format!("{long}file-{index}"))),
            kind: EntryKind::File,
            outcome: Outcome::Completed,
            bytes: 1,
            checksum_verified: true,
            created_directory: false,
            recovery: None,
            trash: None,
            detail: String::new(),
        })
        .collect();
    entries.push(EntryResult {
        source,
        destination: Some(target),
        kind: EntryKind::Directory,
        outcome: Outcome::Completed,
        bytes: 0,
        checksum_verified: false,
        created_directory: true,
        recovery: None,
        trash: None,
        detail: "LAST_RESULT_REACHED".into(),
    });
    let mut app = App::new(root.path().into());
    app.operations.history.push(Job {
        retry_eligible: false,
        id: 1,
        plan,
        options: Default::default(),
        state: JobState::Completed,
        record: Some(Record { entries }),
        progress: None,
        started: SystemTime::UNIX_EPOCH,
        finished: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
        retry_of: None,
    });
    app.operations.show_history = true;
    (root, app)
}

fn draw(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn large_job_details_scroll_beyond_u16_rows_and_reach_the_final_result() {
    let (_root, app) = fixture();
    let start = Instant::now();
    let geometry = ui::geometry(&app, Rect::new(0, 0, 120, 40));
    let geometry_time = start.elapsed();
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    for (code, expected) in [(KeyCode::PageDown, 8), (KeyCode::PageUp, 0)] {
        controller.handle(
            Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            &geometry,
            Duration::ZERO,
        );
        assert_eq!(controller.app.operations.scroll, expected);
    }
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        controller.handle(
            Event::Key(KeyEvent::new_with_kind(
                KeyCode::PageDown,
                KeyModifiers::NONE,
                kind,
            )),
            &geometry,
            Duration::ZERO,
        );
        assert_eq!(
            controller.app.operations.scroll, 0,
            "non-Press paging must be consumed"
        );
    }
    let top = draw(&controller.app);
    assert!(top.contains("PgUp/PgDown"));
    controller.app.operations.scroll = usize::MAX;
    let bottom_start = Instant::now();
    let bottom = draw(&controller.app);
    eprintln!(
        "LARGE_JOB_STRESS: 2500 entries, 40 long path components, geometry {geometry_time:?}, bottom redraw {:?}, total {:?}",
        bottom_start.elapsed(),
        start.elapsed()
    );
    assert!(
        bottom.contains("LAST_RESULT_REACHED"),
        "final result must remain reachable beyond u16 rows"
    );
    assert!(geometry.operation_scroll_limit > usize::from(u16::MAX));
}
