use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Bindings, Controller},
    operations::{EntryResult, JobState, Operation, Outcome, Plan, Record, Update},
    ui,
};

fn fixture() -> (tempfile::TempDir, Controller, EntryResult) {
    let root = tempfile::tempdir().unwrap();
    let path = root
        .path()
        .join(format!("very-long-filename-{}.txt", "界".repeat(80)));
    // Native filenames have byte limits; only the displayed frozen name is long.
    let native = root.path().join("file");
    std::fs::write(&native, vec![0; 1024]).unwrap();
    let mut plan = Plan::paths(Operation::Copy, vec![native], Some(root.path().into())).unwrap();
    plan.operands[0].path = path.clone();
    let result = EntryResult {
        source: path,
        destination: None,
        kind: terminal_file_manager::fs::EntryKind::File,
        outcome: Outcome::Completed,
        bytes: 1024,
        checksum_verified: false,
        created_directory: false,
        detail: String::new(),
        recovery: None,
        trash: None,
    };
    let mut c = Controller::new(
        App::new(root.path().into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    c.register_operation(1, plan, Default::default(), None);
    c.app.operations.show_history = true;
    (root, c, result)
}

fn buffer(app: &App, width: u16, level: ColorLevel) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, 40)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(level));
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn draw(app: &App, width: u16) -> String {
    let buffer = buffer(app, width, ColorLevel::NoColor);
    (0..40)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn pending_roots_do_not_claim_missing_completion() {
    let (_root, mut c, _) = fixture();
    for state in [JobState::Queued, JobState::Running] {
        c.app.operations.history[0].state = state;
        let text = draw(&c.app, 80);
        assert!(!text.contains("no recorded root completion"), "{text}");
        assert!(
            text.lines().any(|line| line.contains("very-long")
                && line.contains(if state == JobState::Queued {
                    "Queued"
                } else {
                    "Running"
                })),
            "{text}"
        );
    }
}

#[test]
fn root_status_survives_truncation_in_monochrome() {
    let (_root, mut c, result) = fixture();
    for (outcome, cue) in [
        (Outcome::Completed, "Done"),
        (Outcome::Failed, "Failed"),
        (Outcome::Skipped, "Skipped"),
        (Outcome::Changed, "Changed"),
        (Outcome::Cancelled, "Cancel"),
        (Outcome::CopiedSourceRetained, "Retained"),
        (Outcome::OutcomeUnknown, "Unknown"),
        (Outcome::SafetyIncident, "CRITICAL"),
    ] {
        let mut entry = result.clone();
        entry.outcome = outcome;
        c.complete_operation(Update::Finished {
            id: 1,
            record: Record {
                entries: vec![entry],
            },
        });
        for width in [24, 40, 80] {
            let text = draw(&c.app, width);
            assert!(
                text.lines()
                    .any(|line| line.contains(cue) && line.contains("…")),
                "{cue}: {text}"
            );
            if width >= 40 {
                assert!(
                    text.lines().any(|line| line.contains(cue)
                        && line.contains("1 KiB")
                        && line.contains("-  ")),
                    "{text}"
                );
            }
        }
    }
}

#[test]
fn final_missing_root_result_is_distinct_from_pending() {
    let (_root, mut c, _) = fixture();
    c.complete_operation(Update::Finished {
        id: 1,
        record: Record::default(),
    });
    assert!(draw(&c.app, 80).contains("Unrecorded"));
}

#[test]
fn large_final_summary_stays_compact_and_refreshes_on_finish() {
    let (_root, mut c, result) = fixture();
    let mut entries: Vec<_> = (0..20_000)
        .map(|i| {
            let mut entry = result.clone();
            entry.source.push(format!("child-{i}"));
            entry.outcome = Outcome::Failed;
            entry.detail = format!("distinct failure {i}");
            entry
        })
        .collect();
    entries.push(result.clone());
    let start = std::time::Instant::now();
    c.complete_operation(Update::Finished {
        id: 1,
        record: Record { entries },
    });
    let finish_time = start.elapsed();
    let redraw_start = std::time::Instant::now();
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, 80, 40));
    assert!(geometry.operation_scroll_limit < 30);
    let text = draw(&c.app, 80);
    assert!(text.contains("20000 failed"), "{text}");
    assert_eq!(
        c.app.operations.history[0]
            .record
            .as_ref()
            .unwrap()
            .entries
            .len(),
        20_001
    );
    for _ in 0..50 {
        assert!(draw(&c.app, 80).contains("20000 failed"));
    }
    eprintln!(
        "20,001 results: finalization {finish_time:?}; 51 redraws {:?}",
        redraw_start.elapsed()
    );
    c.complete_operation(Update::Finished {
        id: 1,
        record: Record {
            entries: vec![result],
        },
    });
    let text = draw(&c.app, 80);
    assert!(text.contains("1 entry copied"), "{text}");
    assert!(!text.contains("distinct failure"));
}

#[test]
fn rejected_request_retains_raw_evidence_and_clips_status_with_details_hint() {
    let (_root, mut c, _) = fixture();
    c.app.operations.show_history = false;
    let detail = format!("multiline\n\u{1b}\u{202e} {} END", "long-error ".repeat(80));
    c.app.status = c.app.operations.fail(detail.clone(), c.app.now);
    c.app.now += std::time::Duration::from_secs(6);
    let text = draw(&c.app, 80);
    assert!(text.contains("F9 details"), "{text}");
    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains('\u{202e}'));
    assert_eq!(
        c.app.operations.last_error.as_deref(),
        Some(detail.as_str())
    );
    c.app.operations.show_history = true;
    let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 40));
    c.app.operations.scroll = g.operation_scroll_limit;
    assert!(draw(&c.app, 80).contains("END"));
}

#[test]
fn healthy_and_incomplete_root_styles_follow_actual_results() {
    let (_root, mut c, result) = fixture();
    for (state, outcome, cue, role) in [
        (JobState::Queued, None, "Queued", Role::Value),
        (JobState::Running, None, "Running", Role::Value),
        (
            JobState::Completed,
            Some(Outcome::Completed),
            "Done",
            Role::Status,
        ),
        (
            JobState::Failed,
            Some(Outcome::Failed),
            "Failed",
            Role::Error,
        ),
        (
            JobState::Partial,
            Some(Outcome::CopiedSourceRetained),
            "Retained",
            Role::Warning,
        ),
        (JobState::Skipped, None, "Unrecorded", Role::Warning),
    ] {
        let job = &mut c.app.operations.history[0];
        job.record = None;
        job.presentation = None;
        if let Some(outcome) = outcome {
            let mut entry = result.clone();
            entry.outcome = outcome;
            job.finish(
                Record {
                    entries: vec![entry],
                },
                c.app.now,
            );
        }
        job.state = state;
        assert_root_style(&c.app, cue, role);
    }
}

fn assert_root_style(app: &App, cue: &str, role: Role) {
    let output = buffer(app, 80, ColorLevel::Ansi256);
    let (x, y) = (0..40)
        .find_map(|y| {
            let line: String = (0..80).map(|x| output[(x, y)].symbol()).collect();
            line.contains("very-long")
                .then(|| (line.find(cue).unwrap() as u16, y))
        })
        .unwrap();
    assert_eq!(
        output[(x, y)].fg,
        Theme::new(ColorLevel::Ansi256)
            .style(role)
            .fg
            .unwrap_or_default()
    );
}
