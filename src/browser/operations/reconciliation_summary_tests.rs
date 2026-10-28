use super::*;
use crate::{color::Theme, operations::QueueTestDriver};
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn queued_and_running_reconciliation_distinguishes_previous_results_until_finish() {
    let (root, mut controller, _unused_queue, plan) = fixture();
    let mut driver = QueueTestDriver::new();
    let id = driver
        .queue
        .submit(plan.clone(), Options::default())
        .unwrap();
    controller.register_operation(id, plan, Options::default(), None);
    driver.pump();
    let held = root.path().join("held-owned");
    let payload = driver.replace_claimed_payload(&held);
    controller.complete_operation(Update::Finished {
        id,
        record: driver.finish(id),
    });
    let old_detail = controller.app.operations.history[0]
        .record
        .as_ref()
        .unwrap()
        .entries[0]
        .detail
        .clone();
    assert!(controller.app.operations.history[0].can_reconcile());
    fs::rename(&payload, root.path().join("foreign-preserved")).unwrap();
    fs::rename(held, &payload).unwrap();
    controller.app.operations.show_history = true;
    key(&mut controller, KeyCode::Char('x'));
    assert_eq!(controller.app.operations.outbox.len(), 1);
    match controller.app.operations.outbox.first() {
        Some(Intent::Reconcile(job)) => assert_eq!(*job, id),
        _ => panic!("X must request reconciliation of the selected job"),
    }
    submit(&mut controller, &mut driver.queue);
    assert_active_reconciliation(&mut controller, JobState::Queued, &old_detail);
    driver.pump();
    start_reconciliation(&mut controller, &mut driver, id);
    assert_active_reconciliation(&mut controller, JobState::Running, &old_detail);
    controller.complete_operation(Update::Finished {
        id,
        record: driver.finish(id),
    });
    assert_final_reconciliation(&mut controller, &root, &payload);
}

fn start_reconciliation(controller: &mut Controller, driver: &mut QueueTestDriver, id: u64) {
    let started = driver
        .queue
        .next(Duration::from_secs(5))
        .expect("reconciliation Started");
    match &started {
        Update::Started { id: started_id } => assert_eq!(*started_id, id),
        _ => panic!("expected reconciliation Started"),
    }
    controller.complete_operation(started);
}

fn assert_active_reconciliation(controller: &mut Controller, state: JobState, detail: &str) {
    let job = &controller.app.operations.history[0];
    assert_eq!(job.state, state);
    assert!(job.presentation.is_some());
    assert_eq!(
        job.record.as_ref().unwrap().entries[0].outcome,
        Outcome::OutcomeUnknown
    );
    assert_eq!(job.record.as_ref().unwrap().entries[0].detail, detail);
    assert!(!job.can_retry() && !job.can_reconcile());
    assert!(controller.app.operations.unknown());
    let text = draw(controller);
    assert!(text.contains("Reconciliation"), "{text}");
    assert!(text.contains(if state == JobState::Queued {
        "Queued"
    } else {
        "Running"
    }));
    let source_row = text.lines().find(|row| row.contains("-  source")).unwrap();
    assert!(
        source_row.contains(if state == JobState::Queued {
            "Queued"
        } else {
            "Running"
        }),
        "{source_row}"
    );
    assert!(!source_row.contains("Unknown") && !source_row.contains("Unrecorded"));
    assert!(text.contains("Previous result"), "{text}");
    assert_previous_evidence(controller, detail);
}

fn assert_previous_evidence(controller: &mut Controller, detail: &str) {
    key(controller, KeyCode::Char('e'));
    assert!(controller.app.operations.jobs_evidence);
    let text = draw(controller);
    assert!(text.contains("Previous result"), "{text}");
    let geometry = crate::ui::geometry(&controller.app, Rect::new(0, 0, 160, 50));
    let mut evidence = String::new();
    for offset in 0..=geometry.operation_scroll_limit {
        controller.app.operations.scroll = offset;
        evidence.push_str(&draw(controller));
    }
    assert!(evidence.contains(detail), "{evidence}");
    assert!(evidence.contains("Owned staging checkpoint"));
    key(controller, KeyCode::Char('e'));
    assert!(!controller.app.operations.jobs_evidence);
    assert_eq!(controller.app.operations.scroll, 0);
}

fn assert_final_reconciliation(
    controller: &mut Controller,
    root: &tempfile::TempDir,
    payload: &Path,
) {
    let job = &controller.app.operations.history[0];
    assert_eq!(job.state, JobState::Failed);
    assert!(!job.record.as_ref().unwrap().unresolved());
    assert!(job.can_retry());
    let text = draw(controller);
    assert!(text.contains("1 failed:"), "{text}");
    assert!(!text.contains("1 outcome unknown:"));
    assert!(!text.contains("1 unknown") && !text.contains("Previous result"));
    assert!(
        text.lines()
            .any(|row| row.contains("Failed") && row.contains("-  source"))
    );
    key(controller, KeyCode::Char('e'));
    controller.app.operations.scroll = usize::MAX;
    assert!(draw(controller).contains("Reconciled retained source"));
    assert!(!payload.parent().unwrap().exists());
    assert_eq!(
        fs::read(root.path().join("source")).unwrap(),
        b"approved bytes"
    );
    assert_eq!(
        fs::read(root.path().join("foreign-preserved")).unwrap(),
        b"foreign payload"
    );
}

fn draw(controller: &Controller) -> String {
    let mut terminal = Terminal::new(TestBackend::new(160, 50)).unwrap();
    terminal
        .draw(|frame| {
            crate::ui::render(frame, &controller.app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    let output = terminal.backend().buffer();
    (0..50)
        .map(|y| {
            (0..160)
                .map(|x| output[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
