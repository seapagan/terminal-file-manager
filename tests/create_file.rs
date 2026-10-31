use crossterm::event::{Event, KeyEvent};
use ratatui::layout::Rect;
use std::{fs, time::Duration};
use terminal_file_manager::{
    app::App,
    input::{Bindings, Controller, chord},
    operations::{Decision, Intent, Outcome, Plan, Update, execute},
    ui,
};

fn loaded() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    let snapshot = terminal_file_manager::fs::list(
        root.path(),
        &terminal_file_manager::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    let mut app = App::new(root.path().into());
    app.complete_listing(0, 1, Ok(snapshot));
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        ),
    )
}

fn submit(c: &mut Controller, name: &str) -> Plan {
    key(c, "Alt+n");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
    c.handle(Event::Paste(name.into()), &g, Duration::ZERO);
    key(c, "Enter");
    let Some(Intent::Submit(plan, options)) = c.app.operations.outbox.pop() else {
        panic!("expected creation plan");
    };
    c.register_operation(
        c.app.operations.history.len() as u64 + 1,
        plan.clone(),
        options,
        None,
    );
    plan
}

fn finish(c: &mut Controller, plan: &Plan) {
    let record = execute(plan, Default::default(), &false.into(), |_| Decision::Skip);
    let id = c.app.operations.history.last().unwrap().id;
    let requests = c.complete_operation(Update::Finished { id, record });
    for request in requests {
        install(c, request);
    }
}

#[test]
fn successful_creation_refreshes_and_highlights_only_the_originating_pane() {
    let (root, mut c) = loaded();
    fs::write(root.path().join("a"), "keep").unwrap();
    let request = c.app.request_refresh(0).unwrap();
    let snapshot = terminal_file_manager::fs::list(
        root.path(),
        &terminal_file_manager::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    c.app.complete_listing(0, request.generation, Ok(snapshot));
    c.app.panes[0].select_all();
    let marks = c.app.panes[0].marks.clone();
    let plan = submit(&mut c, "new");
    c.app.active = 1;
    finish(&mut c, &plan);
    assert_eq!(c.app.panes[0].cursor, Some(root.path().join("new")));
    assert_eq!(c.app.active, 1);
    assert_eq!(c.app.panes[0].marks, marks);
    assert!(c.app.status.contains("1 file created"));
}

#[test]
fn later_cursor_marks_filter_and_navigation_take_precedence_over_creation() {
    for change in [
        "cursor",
        "marks",
        "filter",
        "navigation",
        "failed",
        "pending",
    ] {
        let (root, mut c) = loaded();
        let initial = submit(&mut c, "a");
        finish(&mut c, &initial);
        fs::write(root.path().join("b"), "owned").unwrap();
        let request = c.app.request_refresh(0).unwrap();
        install(&mut c, request);
        let plan = submit(&mut c, "new");
        match change {
            "cursor" => {
                key(&mut c, "Down");
                key(&mut c, "Up");
            }
            "marks" => {
                key(&mut c, "Space");
                key(&mut c, "Space");
            }
            "filter" => c.app.panes[0].set_filter("a".into()),
            "navigation" => {
                let request = c.app.request_listing(0, root.path().into());
                install(&mut c, request);
            }
            "failed" => {
                let request = c.app.request_listing(0, root.path().join("missing"));
                c.app
                    .complete_listing(0, request.generation, Err("not found".into()));
            }
            _ => {
                c.app.request_listing(0, root.path().join("missing"));
            }
        }
        let before = c.app.panes[0].cursor.clone();
        finish(&mut c, &plan);
        assert_eq!(c.app.panes[0].cursor, before, "{change}");
    }
}

fn install(c: &mut Controller, request: terminal_file_manager::app::Request) {
    let snapshot = terminal_file_manager::fs::list(
        &request.path,
        &terminal_file_manager::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    c.app
        .complete_listing(request.pane, request.generation, Ok(snapshot));
}

#[test]
fn creation_highlight_survives_refresh_and_respects_native_visibility() {
    for (name, filter, visible) in [
        ("new", "", true),
        (".hidden", "", cfg!(windows)),
        ("filtered", "wanted", false),
    ] {
        let (root, mut c) = loaded();
        c.app.panes[0].set_filter(filter.into());
        let plan = submit(&mut c, name);
        let request = c.app.request_refresh(0).unwrap();
        install(&mut c, request);
        finish(&mut c, &plan);
        assert!(root.path().join(name).is_file());
        assert_eq!(
            c.app.panes[0].cursor,
            visible.then(|| root.path().join(name))
        );
    }
}

#[test]
fn creation_highlight_survives_a_failed_refresh_after_success() {
    let (root, mut c) = loaded();
    let initial = submit(&mut c, "a");
    finish(&mut c, &initial);
    let plan = submit(&mut c, "new");
    let record = execute(&plan, Default::default(), &false.into(), |_| Decision::Skip);
    let id = c.app.operations.history.last().unwrap().id;
    let requests = c.complete_operation(Update::Finished { id, record });
    for request in requests {
        assert!(c.app.complete_listing(
            request.pane,
            request.generation,
            Err("transient refresh failure".into()),
        ));
    }
    assert_eq!(c.app.panes[0].cursor, Some(root.path().join("a")));
    let request = c.app.request_refresh(0).unwrap();
    install(&mut c, request);
    assert_eq!(c.app.panes[0].cursor, Some(root.path().join("new")));
}

#[test]
fn cancellation_and_collisions_never_select_a_new_file() {
    let (root, mut c) = loaded();
    let plan = submit(&mut c, "new");
    let record = execute(&plan, Default::default(), &true.into(), |_| Decision::Skip);
    assert_eq!(record.entries[0].outcome, Outcome::Cancelled);
    let id = c.app.operations.history.last().unwrap().id;
    let requests = c.complete_operation(Update::Finished { id, record });
    for request in requests {
        install(&mut c, request);
    }
    assert!(!root.path().join("new").exists());
    fs::write(root.path().join("collision"), "preserved").unwrap();
    let plan = submit(&mut c, "collision");
    finish(&mut c, &plan);
    assert_eq!(
        fs::read(root.path().join("collision")).unwrap(),
        b"preserved"
    );
    assert_eq!(
        c.app.operations.history.last().unwrap().state,
        terminal_file_manager::operations::JobState::Failed
    );
}

#[test]
fn invalid_names_and_escape_never_submit_or_mutate() {
    for name in ["", ".", "..", "nested/file", "file/", "file/.", "\0"] {
        let (root, mut c) = loaded();
        key(&mut c, "Alt+n");
        let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
        c.handle(Event::Paste(name.into()), &g, Duration::ZERO);
        let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
        assert!(
            !g.operation_enabled(crossterm::event::KeyCode::Enter),
            "{name:?}"
        );
        key(&mut c, "Enter");
        assert!(c.app.operations.outbox.is_empty());
        key(&mut c, "Esc");
        assert!(c.app.operations.dialog.is_none());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

#[test]
fn escape_cancels_a_valid_file_name_and_a_late_listing_preserves_newer_marks() {
    let (root, mut c) = loaded();
    key(&mut c, "Alt+n");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
    c.handle(Event::Paste("cancelled-file".into()), &g, Duration::ZERO);
    key(&mut c, "Esc");
    assert!(c.app.operations.outbox.is_empty());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    let plan = submit(&mut c, "a");
    finish(&mut c, &plan);
    let plan = submit(&mut c, "new");
    let record = execute(&plan, Default::default(), &false.into(), |_| Decision::Skip);
    let id = c.app.operations.history.last().unwrap().id;
    let requests = c.complete_operation(Update::Finished { id, record });
    key(&mut c, "Space");
    let marks = c.app.panes[0].marks.clone();
    for request in requests {
        install(&mut c, request);
    }
    assert_eq!(c.app.panes[0].cursor, Some(root.path().join("a")));
    assert_eq!(c.app.panes[0].marks, marks);
}

#[test]
fn creation_reuses_the_approved_directory_identity_and_coordinator() {
    let (root, mut c) = loaded();
    let plan = submit(&mut c, "new");
    let mut locks = terminal_file_manager::operations::Coordinator::default();
    assert!(locks.acquire(1, &plan).unwrap());
    assert!(
        !locks
            .acquire(2, &Plan::mkdir(root.path().join("new")))
            .unwrap()
    );
    locks.release(1);
    let retained = root.path().with_extension("retained");
    fs::rename(root.path(), &retained).unwrap();
    fs::create_dir(root.path()).unwrap();
    let record = execute(&plan, Default::default(), &false.into(), |_| Decision::Skip);
    assert_eq!(record.entries[0].outcome, Outcome::Changed);
    assert!(!root.path().join("new").exists());
    assert!(!retained.join("new").exists());
    fs::remove_dir(root.path()).unwrap();
    fs::rename(retained, root.path()).unwrap();
}

#[test]
fn configured_keys_and_footer_click_use_the_same_create_file_action() {
    use terminal_file_manager::input::Action;
    let (_root, c) = loaded();
    let bindings = Bindings::new(&std::collections::BTreeMap::from([
        ("Ctrl+n".into(), Action::CreateFile),
        ("Shift+F7".into(), Action::Help),
    ]))
    .unwrap();
    assert!(
        bindings
            .help_rows()
            .iter()
            .any(|(keys, label)| keys.contains("Ctrl+n") && label == "Create empty file")
    );
    let mut c = Controller::new(c.app, bindings, Default::default());
    key(&mut c, "Ctrl+n");
    assert!(c.app.operations.dialog.is_some());
    key(&mut c, "Esc");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
    let control = g
        .footer_controls
        .iter()
        .find(|control| control.action == Action::CreateFile)
        .unwrap();
    assert_eq!(control.label, "Alt+n File");
    let event = mouse(Rect::new(control.area.right() - 1, control.area.y, 1, 1));
    c.handle(event, &g, Duration::ZERO);
    assert!(c.app.operations.dialog.is_some());
    key(&mut c, "Esc");
    key(&mut c, "Shift+F7");
    assert!(c.app.help && c.app.operations.dialog.is_none());
}

#[test]
fn footer_shortcuts_fit_without_partial_labels_and_use_help_notation() {
    use ratatui::{Terminal, backend::TestBackend};
    use terminal_file_manager::{color::Theme, input::Action};
    let (_root, c) = loaded();
    let theme = Theme::new(colored_text::ColorLevel::TrueColor);
    for (width, new_file_visible) in [(43, false), (44, true), (80, true), (180, true)] {
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
        terminal
            .draw(|f| {
                ui::render(f, &c.app, &theme);
            })
            .unwrap();
        let g = ui::geometry(&c.app, Rect::new(0, 0, width, 24));
        assert_eq!(
            g.footer_controls
                .iter()
                .any(|c| c.action == Action::CreateFile),
            new_file_visible
        );
        for control in &g.footer_controls {
            assert!(control.area.right() <= width);
            let (key, _) = control.label.split_once(' ').unwrap();
            assert!(
                c.app
                    .help_rows
                    .iter()
                    .any(|entry| entry.keys.split(" / ").any(|binding| binding == key))
            );
            let buffer = terminal.backend().buffer();
            let rendered: String = (control.area.x..control.area.right())
                .map(|x| buffer[(x, control.area.y)].symbol())
                .collect();
            assert_eq!(rendered, control.label);
        }
        assert!(
            g.footer_controls
                .windows(2)
                .all(|pair| pair[0].area.right() + 2 == pair[1].area.x)
        );
    }
}

fn mouse(area: Rect) -> Event {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn create_file_uses_shared_modal_geometry_and_mouse_controls_at_small_sizes() {
    use ratatui::{Terminal, backend::TestBackend};
    use terminal_file_manager::{color::Theme, operations::Dialog};
    let (_, mut c) = loaded();
    key(&mut c, "Alt+n");
    for (width, height) in [(120, 40), (80, 24), (40, 16), (20, 10), (8, 5)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| {
                ui::render(f, &c.app, &Theme::new(colored_text::ColorLevel::NoColor));
            })
            .unwrap();
        let g = ui::geometry(&c.app, Rect::new(0, 0, width, height));
        assert!(!g.operation_enabled(crossterm::event::KeyCode::Enter));
        for control in &g.operation_controls {
            assert!(control.area.right() <= width && control.area.bottom() <= height);
        }
        if width >= 40 {
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(text.contains("Create empty file") && text.contains("File name:"));
            assert!(
                text.contains("Parent:")
                    && text.contains("Esc Cancel")
                    && text.contains("Enter Create")
            );
        }
    }
    let Some(Dialog::Operation(d)) = &mut c.app.operations.dialog else {
        panic!("creation dialog")
    };
    d.text = "mouse-file".into();
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
    let button = g
        .operation_controls
        .iter()
        .find(|control| {
            control.target == ui::OperationTarget::Key(crossterm::event::KeyCode::Enter)
        })
        .unwrap();
    c.handle(mouse(button.area), &g, Duration::ZERO);
    assert!(
        matches!(c.app.operations.outbox.last(), Some(Intent::Submit(plan, _)) if plan.destination.as_ref().unwrap().ends_with("mouse-file"))
    );
}

#[test]
#[cfg(windows)]
fn invalid_windows_file_names_disable_create_and_fail_without_mutation() {
    for name in [
        "NUL",
        "con.txt",
        "COM1",
        "CONIN$",
        "conout$",
        "LPT9.txt",
        "file:stream",
        "bad?name",
        "trailing.",
        "trailing ",
    ] {
        let (root, mut c) = loaded();
        key(&mut c, "Alt+n");
        let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
        c.handle(Event::Paste(name.into()), &g, Duration::ZERO);
        let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
        assert!(
            !g.operation_enabled(crossterm::event::KeyCode::Enter),
            "{name}"
        );
        let record = execute(
            &Plan::create_file(root.path().join(name)),
            Default::default(),
            &false.into(),
            |_| Decision::Skip,
        );
        assert_ne!(record.entries[0].outcome, Outcome::Completed, "{name}");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

fn key(c: &mut Controller, input: &str) {
    let (code, modifiers) = chord(input).unwrap();
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
    c.handle(
        Event::Key(KeyEvent::new(code, modifiers)),
        &g,
        Duration::ZERO,
    );
}

#[test]
fn create_file_shortcuts_submit_an_empty_regular_file_from_an_empty_pane() {
    for shortcut in ["Shift+F7", "Alt+n"] {
        let (root, mut c) = loaded();
        key(&mut c, shortcut);
        assert!(
            c.app.operations.dialog.is_some(),
            "{shortcut} must open Create File"
        );
        let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
        c.handle(Event::Paste("資料e\u{301}.txt".into()), &g, Duration::ZERO);
        key(&mut c, "Enter");
        let Some(Intent::Submit(plan, options)) = c.app.operations.outbox.pop() else {
            panic!("expected creation plan");
        };
        assert!(plan.operands.is_empty());
        let record = execute(&plan, options, &false.into(), |_| Decision::Skip);
        assert_eq!(record.entries[0].outcome, Outcome::Completed);
        assert_eq!(record.completed_files(), 1);
        let metadata = fs::symlink_metadata(root.path().join("資料e\u{301}.txt")).unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.len(), 0);
    }
}

#[test]
fn creation_completion_during_an_older_scan_uses_the_deferred_refresh() {
    let (root, mut c) = loaded();
    let plan = submit(&mut c, "new");
    let old_request = c.app.request_refresh(0).unwrap();
    let old_snapshot = terminal_file_manager::fs::list(
        root.path(),
        &terminal_file_manager::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    let record = execute(&plan, Default::default(), &false.into(), |_| Decision::Skip);
    let requests = c.complete_operation(Update::Finished { id: 1, record });
    assert!(requests.is_empty());
    c.app
        .complete_listing(0, old_request.generation, Ok(old_snapshot));
    assert!(c.app.panes[0].cursor.is_none());
    let request = c
        .app
        .take_refresh(0)
        .expect("completion must schedule a fresh scan");
    install(&mut c, request);
    assert_eq!(c.app.panes[0].cursor, Some(root.path().join("new")));
}

#[cfg(windows)]
#[path = "create_file/windows.rs"]
mod windows;
