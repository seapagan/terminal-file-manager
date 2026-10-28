use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Bindings, Controller},
    operations::{
        EntryResult, Job, JobState, Operation, Outcome, Plan, Record, TrashResult,
        trash::{LinuxReceipt, Receipt},
    },
    ui::{self, OperationTarget},
};

fn fixture() -> (tempfile::TempDir, App) {
    let root = tempfile::Builder::new()
        .prefix("tfm-long-native-temporary-directory-")
        .tempdir()
        .unwrap();
    let paths = files(root.path());
    let plan = Plan::paths(Operation::Trash, paths, None).unwrap();
    let record = Record {
        entries: plan
            .operands
            .iter()
            .enumerate()
            .map(|(i, e)| EntryResult {
                source: e.path.clone(),
                destination: Some(root.path().join(format!("unique-trash-{i}"))),
                kind: e.kind,
                outcome: Outcome::Completed,
                bytes: e.size.unwrap(),
                checksum_verified: false,
                created_directory: false,
                recovery: None,
                detail: format!("native validation record {i}"),
                trash: Some(TrashResult {
                    receipt: Receipt::Linux(LinuxReceipt {
                        path: root.path().join(format!("unique-trash-{i}")),
                        info: root.path().join(format!("unique-info-{i}.trashinfo")),
                    }),
                    validated_files: 1,
                    validated_directories: 0,
                }),
            })
            .collect(),
    };
    let mut app = App::new(root.path().into());
    app.operations.history.push(Job {
        presentation: None,
        retry_eligible: false,
        id: 2,
        plan,
        options: Default::default(),
        state: JobState::Completed,
        record: Some(record),
        progress: None,
        started: app.now,
        finished: Some(app.now),
        retry_of: None,
    });
    app.operations.show_history = true;
    (root, app)
}

fn draw(app: &App, width: u16, height: u16) -> (String, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let geometry = ui::geometry(app, Rect::new(0, 0, width, height));
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    (text, geometry)
}

#[test]
fn five_successful_trash_entries_fit_without_forensic_pages() {
    let (_root, app) = fixture();
    let (text, geometry) = draw(&app, 80, 24);
    assert!(text.contains("5 entries trashed"), "{text}");
    assert_eq!(geometry.operation_scroll_limit, 0, "{text}");
    for i in 0..5 {
        assert!(text.contains(&format!("file-{i}.txt")), "{text}");
    }
    assert_eq!(text.matches("From:").count(), 1);
    assert!(!text.contains("To:"));
    for verbose in [
        "trashinfo",
        "unique-trash",
        "Observed bytes",
        "validation record",
        "Results from",
    ] {
        assert!(!text.contains(verbose), "{text}");
    }
}

#[test]
fn evidence_toggle_is_reversible_and_keeps_each_receipt_and_full_source() {
    let (root, app) = fixture();
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let (_, summary) = draw(&c.app, 80, 24);
    key(&mut c, &summary, KeyCode::Char('E'));
    assert!(c.app.operations.jobs_evidence);
    assert!(c.app.operations.outbox.is_empty());
    let (_, evidence) = draw(&c.app, 80, 24);
    assert_eq!(
        summary.operation_controls[0].area,
        evidence.operation_controls[0].area
    );
    for (width, height) in [(40, 15), (80, 24), (120, 40)] {
        let (_, geometry) = draw(&c.app, width, height);
        let mut text = String::new();
        for scroll in 0..=geometry.operation_scroll_limit {
            c.app.operations.scroll = scroll;
            text.push_str(&draw(&c.app, width, height).0);
        }
        let columns = geometry
            .operation_controls
            .iter()
            .find_map(|control| {
                matches!(control.target, OperationTarget::Job(_)).then_some(control.area.width)
            })
            .unwrap();
        assert_receipts(&root, &text, columns);
    }
    click_toggle(&mut c, &evidence);
    assert!(!c.app.operations.jobs_evidence);
    assert_eq!(c.app.operations.scroll, 0);
    assert!(c.app.operations.outbox.is_empty());
    key(&mut c, &summary, KeyCode::Char('e'));
    key(&mut c, &summary, KeyCode::Esc);
    key(&mut c, &summary, KeyCode::F(9));
    assert!(!c.app.operations.jobs_evidence);
    assert_eq!(c.app.operations.scroll, 0);
}

fn key(c: &mut Controller, g: &ui::Geometry, code: KeyCode) {
    c.handle(
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        g,
        Duration::ZERO,
    );
}

#[test]
fn non_success_safety_and_next_action_survive_scrolling_and_narrow_sizes() {
    let (_root, mut app) = fixture();
    for (state, outcome, message, action) in [
        (
            JobState::Failed,
            Outcome::Failed,
            "Failed",
            "correct the cause",
        ),
        (
            JobState::Partial,
            Outcome::CopiedSourceRetained,
            "Partial result",
            "inspect incomplete",
        ),
        (
            JobState::Unknown,
            Outcome::OutcomeUnknown,
            "UNKNOWN",
            "safe exit/retry blocked",
        ),
        (
            JobState::Critical,
            Outcome::SafetyIncident,
            "CRITICAL",
            "stop and report",
        ),
    ] {
        let job = &mut app.operations.history[0];
        job.state = state;
        job.record.as_mut().unwrap().entries[0].outcome = outcome;
        job.record.as_mut().unwrap().entries[0].detail = "recorded failure reason".into();
        for (width, height) in [(40, 15), (80, 24), (120, 40), (300, 100)] {
            for evidence in [false, true] {
                app.operations.jobs_evidence = evidence;
                app.operations.scroll = usize::MAX;
                let (text, _) = draw(&app, width, height);
                assert!(text.contains(message), "{text}");
                if width >= 80 {
                    assert!(text.contains(action), "{text}");
                }
            }
        }
    }
}

#[test]
fn selection_resets_detail_scroll_and_leaves_enter_conflict_routing_intact() {
    use terminal_file_manager::operations::PendingConflict;
    let (_root, app) = fixture();
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let g = draw(&c.app, 80, 24).1;
    key(&mut c, &g, KeyCode::Char('e'));
    c.app.operations.scroll = 8;
    key(&mut c, &g, KeyCode::Down);
    assert_eq!(c.app.operations.scroll, 0);
    c.app.operations.scroll = 8;
    key(&mut c, &g, KeyCode::Up);
    assert_eq!(c.app.operations.scroll, 0);
    let (reply, receiver) = std::sync::mpsc::sync_channel(1);
    let entry = c.app.operations.history[0].plan.operands[0].clone();
    c.app.operations.conflicts.push_back(PendingConflict {
        id: 2,
        choice: 0,
        all: false,
        reply,
        event: terminal_file_manager::operations::Event::Conflict {
            source: Box::new(entry.clone()),
            destination: Box::new(entry),
            newer_available: false,
        },
    });
    key(&mut c, &g, KeyCode::Enter);
    assert!(!c.app.operations.show_history);
    assert_eq!(c.app.operations.conflicts.len(), 1);
    assert!(receiver.try_recv().is_err());
    assert!(c.app.operations.outbox.is_empty());
}

#[test]
fn source_root_elision_keeps_rows_relative_and_native_identities_unchanged() {
    let (_root, mut app) = fixture();
    let root = std::path::PathBuf::from(
        "/home/資料/agent-scratch/very-long-investigation/session/visual/destination",
    );
    let job = &mut app.operations.history[0];
    for (operand, result) in job
        .plan
        .operands
        .iter_mut()
        .zip(&mut job.record.as_mut().unwrap().entries)
    {
        operand.path = root.join(&operand.name);
        result.source = operand.path.clone();
    }
    let (text, geometry) = draw(&app, 80, 24);
    assert!(text.contains("…"), "{text}");
    assert!(text.contains("destination"), "{text}");
    assert_eq!(text.matches("From:").count(), 1);
    assert_eq!(geometry.operation_scroll_limit, 0);
    let frozen = app.operations.history[0].plan.operands[0].path.clone();
    app.operations.jobs_evidence = true;
    let mut evidence = String::new();
    for scroll in 0..=draw(&app, 80, 24).1.operation_scroll_limit {
        app.operations.scroll = scroll;
        evidence.push_str(&draw(&app, 80, 24).0);
    }
    assert!(evidence.contains("very-long-investigation"));
    assert_eq!(app.operations.history[0].plan.operands[0].path, frozen);
    assert!(app.operations.outbox.is_empty());
}

#[test]
fn summary_does_not_describe_links_as_regular_files_or_show_their_size() {
    let (_root, mut app) = fixture();
    let job = &mut app.operations.history[0];
    job.plan.operands.truncate(1);
    job.plan.operands[0].kind = terminal_file_manager::fs::EntryKind::Link;
    job.record.as_mut().unwrap().entries.truncate(1);
    job.record.as_mut().unwrap().entries[0].kind = terminal_file_manager::fs::EntryKind::Link;
    let (text, _) = draw(&app, 80, 24);
    assert!(text.contains("1 entry trashed"));
    assert!(text.contains("@  file-0.txt"), "{text}");
    assert!(!text.contains("KiB"));
    assert!(!text.contains("1 files"));
}

fn assert_receipts(root: &tempfile::TempDir, text: &str, columns: u16) {
    // Synthetic Linux receipts exercise presentation on every host, not native Trash.
    for i in 0..5 {
        for line in [
            format!(
                "  Native Trash target: {}",
                root.path().join(format!("unique-trash-{i}")).display()
            ),
            format!(
                "  Recovery metadata: {}",
                root.path()
                    .join(format!("unique-info-{i}.trashinfo"))
                    .display()
            ),
            format!("  native validation record {i}"),
            format!(
                "  Source: {}",
                root.path().join(format!("file-{i}.txt")).display()
            ),
        ] {
            // Check every displayed cell of each full line, including wrapped paths.
            let cells: Vec<_> = ui::clean(&line).chars().collect();
            for chunk in cells.chunks(usize::from(columns)) {
                let expected: String = chunk.iter().collect();
                assert!(
                    text.contains(&expected),
                    "missing {expected:?} in {columns} columns"
                );
            }
        }
    }
}

fn click_toggle(c: &mut Controller, evidence: &ui::Geometry) {
    let toggle = evidence
        .operation_controls
        .iter()
        .find(|b| b.target == OperationTarget::Key(KeyCode::Char('e')))
        .unwrap();
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: toggle.area.x,
            row: toggle.area.y,
            modifiers: KeyModifiers::NONE,
        }),
        evidence,
        Duration::ZERO,
    );
}

fn files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    (0..5)
        .map(|i| {
            let path = root.join(format!("file-{i}.txt"));
            std::fs::write(&path, vec![0; (i + 1) * 1024]).unwrap();
            path
        })
        .collect()
}

#[cfg(unix)]
#[test]
fn evidence_preserves_invalid_native_bytes_and_escapes_terminal_controls() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let (_root, mut app) = fixture();
    let native = std::path::PathBuf::from(OsString::from_vec(b"/source/bad-\xff\x1b".to_vec()));
    let record = app.operations.history[0].record.as_mut().unwrap();
    record.entries[0].source = native.clone();
    record.entries[0].outcome = Outcome::Failed;
    record.entries[0].detail = "recorded error \u{1b} \u{202e}".into();
    app.operations.history[0].state = JobState::Partial;
    app.operations.jobs_evidence = true;
    let mut all = String::new();
    for scroll in 0..=draw(&app, 120, 40).1.operation_scroll_limit {
        app.operations.scroll = scroll;
        all.push_str(&draw(&app, 120, 40).0);
    }
    assert!(all.contains("bad-\\xFF"), "{all}");
    assert!(all.contains("recorded error"));
    assert!(all.contains("\\u{202e}"));
    assert!(!all.contains('\u{1b}'));
    assert_eq!(
        app.operations.history[0].record.as_ref().unwrap().entries[0].source,
        native
    );
}

#[test]
fn foreign_source_roots_never_gain_a_fake_common_parent() {
    let (_root, mut app) = fixture();
    let job = &mut app.operations.history[0];
    job.plan.operands.truncate(2);
    job.plan.operands[0].path = std::path::PathBuf::from(r"C:\one\first.txt");
    job.plan.operands[1].path = std::path::PathBuf::from(r"D:\two\second.txt");
    let (text, _) = draw(&app, 120, 40);
    assert!(!text.contains("From:"), "{text}");
    assert!(text.contains(r"C:\one\first.txt"));
    assert!(text.contains(r"D:\two\second.txt"));
}
