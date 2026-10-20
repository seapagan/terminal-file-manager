use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::{fs, time::Duration};
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs::IgnoreContext,
    input::{Action, Bindings, Controller},
    operations::{Intent, Outcome, Update},
    ui,
};

fn controller() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("left")).unwrap();
    fs::create_dir(root.path().join("right")).unwrap();
    fs::write(root.path().join("left/one"), "one").unwrap();
    fs::write(root.path().join("left/two"), "two").unwrap();
    let mut app = App::new(root.path().join("left"));
    for (id, name) in [(0, "left"), (1, "right")] {
        let snapshot = terminal_file_manager::fs::list(
            &root.path().join(name),
            &IgnoreContext::default(),
            &false.into(),
        )
        .unwrap();
        app.complete_listing(id, 1, Ok(snapshot));
    }
    let bindings = Bindings::new(&Default::default()).unwrap();
    (root, Controller::new(app, bindings, Default::default()))
}

fn key(controller: &mut Controller, code: KeyCode) {
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.handle(
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        &geometry,
        Duration::ZERO,
    );
}

#[test]
fn smart_copy_freezes_dual_pane_single_operand_and_empty_scope_never_submits() {
    let (root, mut controller) = controller();
    controller.app.dual = true;
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::Copy, &geometry);
    assert!(controller.app.operations.dialog.is_none());
    let Intent::Submit(plan, _) = controller.app.operations.outbox.pop().unwrap() else {
        panic!("expected frozen submission");
    };
    assert_eq!(plan.operands[0].path, root.path().join("left/one"));
    assert_eq!(plan.destination, Some(root.path().join("right")));
    controller.app.panes[0].set_filter("zero matches".into());
    controller.dispatch(Action::Copy, &geometry);
    assert!(controller.app.operations.outbox.is_empty());
    assert!(controller.app.operations.dialog.is_none());
}

#[test]
fn multi_confirmation_refreshes_changed_scope_and_requires_fresh_approval() {
    let (_, mut controller) = controller();
    controller.app.dual = true;
    controller.app.panes[0].select_all();
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::Copy, &geometry);
    assert!(controller.app.operations.dialog.is_some());
    controller.app.panes[0].set_filter("two".into());
    key(&mut controller, KeyCode::Enter);
    assert!(controller.app.operations.outbox.is_empty());
    assert!(controller.app.status.contains("changed"));
    key(&mut controller, KeyCode::Enter);
    let Intent::Submit(plan, _) = controller.app.operations.outbox.pop().unwrap() else {
        panic!("expected submission");
    };
    assert_eq!(plan.operands.len(), 1);
    assert_eq!(plan.operands[0].name, "two");
}

#[test]
fn permanent_delete_defaults_to_cancel_and_renders_a_textual_danger_warning() {
    let (_, mut controller) = controller();
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::Delete, &geometry);
    assert!(matches!(
        controller.app.operations.outbox.pop(),
        Some(Intent::Preview(_))
    ));
    let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(
                frame,
                &controller.app,
                &Theme::new(colored_text::ColorLevel::NoColor),
            );
        })
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("PERMANENT DELETE"));
    assert!(text.contains("Cancel"));
    key(&mut controller, KeyCode::Enter);
    assert!(controller.app.operations.outbox.is_empty());
    assert!(controller.app.operations.dialog.is_none());
}

fn outcome(
    controller: &Controller,
    outcome: terminal_file_manager::operations::Outcome,
) -> terminal_file_manager::operations::Record {
    let source = controller.app.panes[0].entries[0].clone();
    terminal_file_manager::operations::Record {
        entries: vec![terminal_file_manager::operations::EntryResult {
            recovery: None,
            source: source.path,
            destination: None,
            kind: source.kind,
            outcome,
            bytes: 0,
            checksum_verified: false,
            created_directory: false,
            detail: "controlled worker acknowledgement".into(),
        }],
    }
}

fn running(controller: &mut Controller) {
    let plan = terminal_file_manager::operations::Plan::capture(
        &controller.app.panes[0],
        terminal_file_manager::operations::Operation::Copy,
        Some(controller.app.panes[1].path.clone()),
    )
    .unwrap();
    controller.register_operation(1, plan, Default::default(), None);
}

#[test]
fn cancel_then_exit_waits_for_acknowledgement_and_return_abandons_exit_intent() {
    let (_, mut controller) = controller();
    running(&mut controller);
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::SaveQuit, &geometry);
    assert!(!controller.quit);
    key(&mut controller, KeyCode::Enter);
    assert!(controller.app.operations.exit_snapshot.is_none());
    assert!(!controller.save_session);
    controller.dispatch(Action::Quit, &geometry);
    key(&mut controller, KeyCode::Char('c'));
    assert!(matches!(
        controller.app.operations.outbox.last(),
        Some(Intent::Cancel(1))
    ));
    assert!(!controller.quit);
    finish(&mut controller, Outcome::Cancelled);
    assert!(controller.quit);
    assert!(!controller.save_session);
}

#[test]
fn save_quit_freezes_navigation_until_safe_wait_finishes() {
    let (root, mut controller) = controller();
    running(&mut controller);
    let before = controller.app.panes[0].path.clone();
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::SaveQuit, &geometry);
    key(&mut controller, KeyCode::Char('w'));
    assert!(!controller.quit);
    controller.app.panes[0].path = root.path().join("later-navigation");
    finish(&mut controller, Outcome::Completed);
    assert!(controller.quit && controller.save_session);
    let snapshot = controller.app.operations.exit_snapshot.as_ref().unwrap();
    assert_eq!(snapshot.locations().unwrap()[0], before);
    let store = terminal_file_manager::session::SessionStore {
        path: root.path().join("saved.toml"),
    };
    store.save_snapshot(snapshot).unwrap();
    assert_eq!(store.load().unwrap().locations().unwrap()[0], before);
}

#[test]
fn an_unknown_outcome_blocks_retry_and_safe_exit_without_freezing_the_manager() {
    let (_, mut controller) = controller();
    running(&mut controller);
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::Quit, &geometry);
    key(&mut controller, KeyCode::Char('c'));
    finish(&mut controller, Outcome::OutcomeUnknown);
    assert!(!controller.quit);
    assert!(controller.app.operations.waiting_exit.is_none());
    assert!(controller.app.status.contains("unknown"));
    let job = &controller.app.operations.history[0];
    assert!(job.plan.retry(job.record.as_ref().unwrap()).is_err());
}

#[cfg(unix)]
#[test]
fn editing_an_incoming_name_keeps_an_unedited_non_utf8_target_exact() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let (root, mut controller) = controller();
    let native = root
        .path()
        .join(OsString::from_vec(b"target-\xff".to_vec()));
    fs::create_dir(&native).unwrap();
    controller.app.panes[1].path = native.clone();
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::Copy, &geometry);
    key(&mut controller, KeyCode::Tab);
    for character in "renamed".chars() {
        key(&mut controller, KeyCode::Char(character));
    }
    key(&mut controller, KeyCode::Enter);
    let Some(Intent::Submit(plan, _)) = controller.app.operations.outbox.pop() else {
        panic!("native target submission");
    };
    assert_eq!(plan.destination, Some(native));
    assert_eq!(plan.incoming_name, Some("renamed".into()));
}

#[cfg(unix)]
#[test]
fn queued_operands_do_not_follow_a_changed_pane_namespace_even_to_the_same_objects() {
    for change_destination in [false, true] {
        let (root, mut controller) = controller();
        controller.app.dual = true;
        let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
        controller.dispatch(Action::Copy, &geometry);
        let Some(Intent::Submit(plan, options)) = controller.app.operations.outbox.pop() else {
            panic!("queued copy");
        };
        let name = if change_destination { "right" } else { "left" };
        let retained = format!("retained-{name}");
        fs::rename(root.path().join(name), root.path().join(&retained)).unwrap();
        std::os::unix::fs::symlink(&retained, root.path().join(name)).unwrap();
        let record =
            terminal_file_manager::operations::execute(&plan, options, &false.into(), |_| {
                terminal_file_manager::operations::Decision::Skip
            });
        assert_eq!(record.entries[0].outcome, Outcome::Changed);
        assert!(!root.path().join("right/one").exists());
    }
}

fn finish(controller: &mut Controller, value: Outcome) {
    let record = outcome(controller, value);
    controller.complete_operation(Update::Finished { id: 1, record });
}

#[test]
fn jobs_reconciliation_submits_the_original_unknown_job_identity() {
    let (_, mut controller) = controller();
    running(&mut controller);
    finish(&mut controller, Outcome::OutcomeUnknown);
    controller.app.operations.show_history = true;
    key(&mut controller, KeyCode::Char('x'));
    assert!(matches!(
        controller.app.operations.outbox.last(),
        Some(Intent::Reconcile(1))
    ));
    assert_eq!(controller.app.operations.history.len(), 1);
    assert!(!controller.quit);
}

#[test]
fn conflict_arrow_choice_and_scroll_use_the_visible_operation_geometry() {
    let (_, mut controller) = controller();
    let source = controller.app.panes[0].entries[0].clone();
    let destination = controller.app.panes[0].entries[1].clone();
    let (reply, response) = std::sync::mpsc::sync_channel(1);
    controller.complete_operation(Update::Conflict {
        id: 1,
        event: terminal_file_manager::operations::Event::Conflict {
            source: Box::new(source),
            destination: Box::new(destination),
            newer_available: true,
        },
        reply,
    });
    for _ in 0..10 {
        key(&mut controller, KeyCode::PageDown);
    }
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    assert!(controller.app.operations.scroll <= usize::from(geometry.operation_scroll_limit));
    key(&mut controller, KeyCode::Down);
    key(&mut controller, KeyCode::Enter);
    assert!(matches!(
        response.try_recv().unwrap(),
        terminal_file_manager::operations::Decision::Replace
    ));
}

#[test]
fn completed_job_elapsed_time_stops_at_worker_acknowledgement() {
    let (_, mut controller) = controller();
    running(&mut controller);
    controller.app.now += Duration::from_secs(7);
    finish(&mut controller, Outcome::Completed);
    controller.app.now += Duration::from_secs(100);
    controller.app.operations.show_history = true;
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(
                frame,
                &controller.app,
                &Theme::new(colored_text::ColorLevel::NoColor),
            );
        })
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("elapsed ~7s"), "{text}");
}
