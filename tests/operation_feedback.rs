use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend};
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Bindings, Controller},
    operations::{Decision, Operation, Plan, Record, Update, execute},
    ui,
};

fn draw(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(160, 50)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    (0..50)
        .map(|y| {
            (0..160)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn finish(plan: Plan, record: Record) -> Controller {
    let mut c = Controller::new(
        App::new(plan.operands[0].path.parent().unwrap().into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    c.register_operation(1, plan, Default::default(), None);
    c.complete_operation(Update::Finished { id: 1, record });
    c
}

#[test]
fn same_folder_copy_and_move_explain_failure_without_mutating_operands() {
    for operation in [Operation::Copy, Operation::Move] {
        for count in [1, 6] {
            let root = tempfile::tempdir().unwrap();
            let paths: Vec<_> = (0..count)
                .map(|i| {
                    let path = root.path().join(format!("selected-{i}.txt"));
                    std::fs::write(&path, b"retained").unwrap();
                    path
                })
                .collect();
            let plan = Plan::paths(operation, paths.clone(), Some(root.path().into())).unwrap();
            let record = execute(&plan, Default::default(), &false.into(), |_| Decision::Skip);
            assert!(
                record
                    .entries
                    .iter()
                    .all(|e| e.detail == "Source and destination are the same.")
            );
            assert_same_folder_feedback(finish(plan, record), count, paths);
        }
    }
}

fn assert_same_folder_feedback(mut c: Controller, count: usize, paths: Vec<std::path::PathBuf>) {
    assert!(
        c.app
            .status
            .contains("Source and destination are the same.")
    );
    assert!(draw(&c.app).contains("Source and destination are the same."));
    c.app.operations.show_history = true;
    let summary = draw(&c.app)
        .lines()
        .filter(|row| row.contains('│'))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        summary
            .matches("Source and destination are the same.")
            .count(),
        1,
        "{summary}"
    );
    assert!(summary.contains(&format!("{count} failed")), "{summary}");
    assert!(!summary.contains("(E details)"));
    c.app.operations.jobs_evidence = true;
    let g = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 160, 50));
    let mut evidence = String::new();
    for offset in 0..=g.operation_scroll_limit {
        c.app.operations.scroll = offset;
        evidence.push_str(&draw(&c.app));
    }
    for path in paths {
        assert_eq!(std::fs::read(&path).unwrap(), b"retained");
        assert!(evidence.contains(path.to_str().unwrap()));
    }
    assert!(evidence.contains("Source and destination are the same."));
}

fn evidence(c: &mut Controller) -> String {
    c.app.operations.jobs_evidence = true;
    let g = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 160, 50));
    let mut text = String::new();
    for offset in 0..=g.operation_scroll_limit {
        c.app.operations.scroll = offset;
        text.push_str(&draw(&c.app));
    }
    text
}

fn summary_text(c: &Controller) -> String {
    draw(&c.app)
        .lines()
        .filter(|row| row.contains('│'))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn mixed_native_success_and_failure_never_claims_nothing_was_copied() {
    for operation in [Operation::Copy, Operation::Move] {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("other")).unwrap();
        let invalid = root.path().join("invalid");
        let valid = root.path().join("other/valid");
        std::fs::write(&invalid, b"retained").unwrap();
        std::fs::write(&valid, b"incoming").unwrap();
        let plan = Plan::paths(
            operation,
            vec![invalid.clone(), valid.clone()],
            Some(root.path().into()),
        )
        .unwrap();
        let record = execute(&plan, Default::default(), &false.into(), |_| Decision::Skip);
        let mut c = finish(plan, record);
        assert!(c.app.status.contains("1 failed"));
        assert!(c.app.status.contains(if operation == Operation::Copy {
            "1 file copied"
        } else {
            "1 file moved"
        }));
        c.app.operations.show_history = true;
        assert!(summary_text(&c).contains("1 failed: Source and destination are the same."));
        assert_eq!(std::fs::read(invalid).unwrap(), b"retained");
        assert_eq!(
            std::fs::read(root.path().join("valid")).unwrap(),
            b"incoming"
        );
        assert_eq!(valid.exists(), operation == Operation::Copy);
    }
}

#[test]
fn grouped_causes_and_all_outcomes_remain_distinct_with_full_evidence() {
    use terminal_file_manager::operations::JobState;
    let (_root, mut c, paths) = mixed_outcome_fixture();
    assert_eq!(c.app.operations.history[0].state, JobState::Unknown);
    assert!(c.app.status.starts_with("Lost reply"));
    for count in [
        "1 file copied",
        "1 skipped",
        "3 failed",
        "1 changed",
        "1 source retained",
        "1 unknown",
    ] {
        assert!(c.app.status.contains(count), "{}", c.app.status);
    }
    c.app.operations.show_history = true;
    let summary = summary_text(&c);
    assert_eq!(
        summary.matches("2 failed: Access denied").count(),
        1,
        "{summary}"
    );
    assert_eq!(
        summary.matches("1 failed: Read only").count(),
        1,
        "{summary}"
    );
    assert!(summary.contains("UNKNOWN"));
    assert!(!summary.contains("R Retry"));
    assert!(!summary.contains("(E details)"));
    let all = evidence(&mut c);
    for reason in [
        "Access denied",
        "Read only",
        "Lost reply",
        "Removal refused",
        "Object changed",
    ] {
        assert!(all.contains(reason));
    }
    for path in paths {
        assert!(all.contains(path.to_str().unwrap()));
    }
}

#[test]
fn rejected_request_notice_keeps_its_reason_and_does_not_invent_a_job() {
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(root.path().into());
    app.operations
        .fail("Destination is unavailable.".into(), app.now);
    let text = draw(&app);
    assert!(text.contains("Not started: Destination is unavailable."));
    assert!(app.operations.history.is_empty());
    app.operations.show_history = true;
    assert!(draw(&app).contains("Latest request not started: Destination is unavailable."));
}

fn mixed_outcome_fixture() -> (tempfile::TempDir, Controller, Vec<std::path::PathBuf>) {
    use terminal_file_manager::operations::Outcome;
    let root = tempfile::tempdir().unwrap();
    let paths: Vec<_> = (0..8)
        .map(|i| {
            let p = root.path().join(format!("file-{i}"));
            std::fs::write(&p, b"owned").unwrap();
            p
        })
        .collect();
    let plan = Plan::paths(Operation::Copy, paths.clone(), Some(root.path().into())).unwrap();
    let mut record = execute(&plan, Default::default(), &false.into(), |_| Decision::Skip);
    for (entry, (outcome, reason)) in record.entries.iter_mut().zip([
        (Outcome::Completed, ""),
        (Outcome::Skipped, ""),
        (Outcome::Failed, "Access denied"),
        (Outcome::Failed, "Access denied"),
        (Outcome::Failed, "Read only"),
        (Outcome::OutcomeUnknown, "Lost reply"),
        (Outcome::CopiedSourceRetained, "Removal refused"),
        (Outcome::Changed, "Object changed"),
    ]) {
        entry.outcome = outcome;
        entry.detail = reason.into();
    }
    (root, finish(plan, record), paths)
}
