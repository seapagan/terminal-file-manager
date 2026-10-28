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
            trash: None,
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

#[cfg(target_os = "linux")]
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
fn operation_surfaces_consume_navigation_repeats_without_moving_the_pane() {
    for history in [false, true] {
        for waiting in [false, true] {
            let (_, mut controller) = controller();
            let (reply, response) = std::sync::mpsc::sync_channel(1);
            controller.complete_operation(Update::Conflict {
                id: 1,
                event: terminal_file_manager::operations::Event::Conflict {
                    source: Box::new(controller.app.panes[0].entries[0].clone()),
                    destination: Box::new(controller.app.panes[0].entries[1].clone()),
                    newer_available: true,
                },
                reply,
            });
            controller.app.operations.show_history = history;
            if history {
                controller.app.operations.conflicts.clear();
            }
            controller.app.operations.waiting_exit = waiting.then_some(false);
            assert_modal_navigation(&mut controller);
            assert_eq!(
                response.try_recv().unwrap_err(),
                if history {
                    std::sync::mpsc::TryRecvError::Disconnected
                } else {
                    std::sync::mpsc::TryRecvError::Empty
                }
            );
        }
    }
}

fn assert_modal_navigation(controller: &mut Controller) {
    use crossterm::event::KeyEventKind;
    for code in [
        KeyCode::Char('j'),
        KeyCode::Down,
        KeyCode::Char('k'),
        KeyCode::Up,
    ] {
        let index = usize::from(matches!(code, KeyCode::Up | KeyCode::Char('k')));
        controller.app.panes[0].cursor = Some(controller.app.panes[0].entries[index].path.clone());
        let before = controller.app.panes[0].cursor.clone();
        for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
            let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
            controller.handle(
                Event::Key(KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind)),
                &geometry,
                Duration::ZERO,
            );
            assert_eq!(controller.app.panes[0].cursor, before, "{code:?} {kind:?}");
        }
    }
}

#[test]
fn nonmodal_navigation_still_accepts_key_repeat() {
    let (_, mut controller) = controller();
    let before = controller.app.panes[0].cursor.clone();
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.handle(
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::Down,
            KeyModifiers::NONE,
            crossterm::event::KeyEventKind::Repeat,
        )),
        &geometry,
        Duration::ZERO,
    );
    assert_ne!(controller.app.panes[0].cursor, before);
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
    assert!(controller.app.operations.scroll <= geometry.operation_scroll_limit);
    key(&mut controller, KeyCode::Down);
    key(&mut controller, KeyCode::Enter);
    assert!(matches!(
        response.try_recv().unwrap(),
        terminal_file_manager::operations::Decision::Replace
    ));
}

#[path = "operation_controls/lifecycle.rs"]
mod lifecycle;
#[path = "operation_controls/preview.rs"]
mod preview;
