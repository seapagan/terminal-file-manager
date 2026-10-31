//! Context Help is a separate read-only focus layer over the measured parent.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Bindings, Controller, chord},
    ui,
};

fn controller(app: App) -> Controller {
    Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}

fn key(c: &mut Controller, text: &str) {
    let (code, modifiers) = chord(text).unwrap();
    let g = ui::geometry(&c.app, Rect::new(0, 0, 100, 32));
    assert!(
        c.handle(
            Event::Key(KeyEvent::new(code, modifiers)),
            &g,
            Duration::ZERO
        )
        .is_empty()
    );
}

fn draw(c: &Controller) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 32)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, &c.app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .chunks(100)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn f1_opens_pattern_help_and_escape_restores_literal_question_and_focus() {
    let mut c = controller(App::new("owned-context-help".into()));
    key(&mut c, "Alt+b");
    key(&mut c, "?");
    assert_eq!(c.app.pattern.as_ref().unwrap().expression(), "?");
    let focus = c.app.pattern.as_ref().unwrap().focus;
    key(&mut c, "F1");
    assert!(draw(&c).contains("Pattern Help"));
    for key_name in ["Enter", "Alt+m", "Tab", "F5", "?"] {
        key(&mut c, key_name);
    }
    assert_eq!(c.app.pattern.as_ref().unwrap().expression(), "?");
    assert_eq!(c.app.pattern.as_ref().unwrap().focus, focus);
    assert!(c.app.operations.outbox.is_empty());
    key(&mut c, "Esc");
    assert!(c.app.pattern.is_some());
    assert_eq!(c.app.pattern.as_ref().unwrap().expression(), "?");
    key(&mut c, "Esc");
    assert!(c.app.pattern.is_none());
}

#[test]
fn context_help_blocks_operation_submission_paste_and_parent_button_clicks() {
    use terminal_file_manager::operations::{Dialog, Plan};
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(dialog_fixture::dialog(Plan::create_file(
        root.path().into(),
    )));
    key(&mut c, "n");
    let before = ui::geometry(&c.app, Rect::new(0, 0, 100, 32));
    let button = before
        .operation_controls
        .iter()
        .find(|control| control.label == "Enter Create")
        .unwrap()
        .area;
    key(&mut c, "F1");
    assert!(draw(&c).contains("Create File Help"));
    key(&mut c, "Enter");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 100, 32));
    c.handle(Event::Paste("other".into()), &g, Duration::ZERO);
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: button.x,
            row: button.y,
            modifiers: KeyModifiers::NONE,
        }),
        &g,
        Duration::ZERO,
    );
    assert!(c.app.operations.outbox.is_empty());
    key(&mut c, "Esc");
    let Some(Dialog::Operation(dialog)) = &c.app.operations.dialog else {
        panic!("parent retained");
    };
    assert_eq!(dialog.text, "n");
    let after = ui::geometry(&c.app, Rect::new(0, 0, 100, 32));
    assert_eq!(
        before
            .operation_controls
            .iter()
            .map(|c| c.area)
            .collect::<Vec<_>>(),
        after
            .operation_controls
            .iter()
            .map(|c| c.area)
            .collect::<Vec<_>>()
    );
}

mod dialog_fixture;

#[test]
fn title_help_click_restores_pattern_mode_case_scroll_and_geometry() {
    let mut c = controller(App::new("owned-context-help".into()));
    key(&mut c, "Alt+b");
    key(&mut c, "Alt+x");
    key(&mut c, "Alt+s");
    key(&mut c, "?");
    let parent = c.app.pattern.as_mut().unwrap();
    parent.scroll = 1;
    let before = (
        parent.expression().to_owned(),
        parent.mode,
        parent.case_sensitive,
        parent.focus,
        parent.scroll,
    );
    let g = ui::geometry(&c.app, Rect::new(0, 0, 100, 32));
    let area = g.context_control.unwrap();
    let parent_shell = g.pattern.unwrap().shell;
    let mouse = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.right() - 1,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    });
    c.handle(
        mouse,
        &ui::geometry(&c.app, Rect::new(0, 0, 100, 32)),
        Duration::ZERO,
    );
    assert!(c.app.context_help.is_some());
    key(&mut c, "End");
    key(&mut c, "Alt+w");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 100, 32));
    assert_eq!(g.pattern.unwrap().shell.outer, parent_shell.outer);
    key(&mut c, "Esc");
    let parent = c.app.pattern.as_ref().unwrap();
    assert_eq!(
        (
            parent.expression().to_owned(),
            parent.mode,
            parent.case_sensitive,
            parent.focus,
            parent.scroll
        ),
        before
    );
}

#[test]
fn jobs_and_metadata_help_do_not_change_their_parent_state() {
    for metadata in [false, true] {
        let mut c = controller(App::new("owned-context-help".into()));
        if metadata {
            key(&mut c, "Alt+Enter");
            c.app.overlay_scroll = 2;
        } else {
            c.app.operations.show_history = true;
            c.app.operations.jobs_evidence = true;
            c.app.operations.scroll = 3;
        }
        key(&mut c, "F1");
        assert!(draw(&c).contains(if metadata {
            "Metadata Help"
        } else {
            "Jobs Help"
        }));
        for input in ["e", "E", "r", "c", "Enter", "q"] {
            key(&mut c, input);
        }
        assert!(!c.quit && c.app.operations.outbox.is_empty());
        key(&mut c, "Esc");
        if metadata {
            assert!(c.app.popup);
            assert_eq!(c.app.overlay_scroll, 2);
        } else {
            assert!(c.app.operations.show_history && c.app.operations.jobs_evidence);
            assert_eq!(c.app.operations.scroll, 3);
        }
    }
}

#[test]
fn conflict_help_preserves_apply_all_choice_and_sends_no_decision() {
    let (_root, mut app, response) = conflict_fixture::fixture();
    conflict_fixture::edit(&mut app, |_, _, newer| *newer = false);
    let mut c = controller(app);
    key(&mut c, "a");
    key(&mut c, "Down");
    let choice = c.app.operations.conflicts.front().unwrap().choice;
    key(&mut c, "F1");
    let (_, _, text) = conflict_fixture::draw(&c.app, 100, 40, ColorLevel::NoColor);
    assert!(text.contains("Conflict Help") && text.contains("a Apply to all"));
    for input in ["a", "A", "r", "R", "Enter", "F9"] {
        key(&mut c, input);
    }
    assert!(response.try_recv().is_err());
    key(&mut c, "Esc");
    let conflict = c.app.operations.conflicts.front().unwrap();
    assert!(conflict.all);
    assert_eq!(conflict.choice, choice);
    assert!(response.try_recv().is_err());
    assert!(
        !ui::geometry(&c.app, Rect::new(0, 0, 100, 32))
            .operation_enabled(crossterm::event::KeyCode::Char('n'))
    );
}

#[test]
fn narrow_context_help_has_f1_access_and_clamps_its_own_scroll_only() {
    for width in [1, 20, 40, 80, 120] {
        let mut c = controller(App::new("owned-context-help".into()));
        key(&mut c, "Alt+b");
        key(&mut c, "F1");
        c.app.context_help.as_mut().unwrap().scroll = u16::MAX;
        let bounds = Rect::new(0, 0, width, 12);
        let g = ui::geometry(&c.app, bounds);
        let limit = g.context_help.as_ref().unwrap().scroll_limit;
        c.handle(Event::Resize(width, 12), &g, Duration::ZERO);
        assert_eq!(c.app.context_help.as_ref().unwrap().scroll, limit);
        let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
        terminal
            .draw(|f| {
                ui::render(f, &c.app, &Theme::new(ColorLevel::NoColor));
            })
            .unwrap();
        assert!(c.app.pattern.is_some());
        assert!(c.app.operations.outbox.is_empty());
    }
}

#[test]
fn configured_metadata_keys_appear_in_context_and_emergency_exit_still_works() {
    use terminal_file_manager::input::Action;
    let mut c = Controller::new(
        App::new("owned-context-help".into()),
        Bindings::new(&std::collections::BTreeMap::from([
            ("c".into(), Action::Help),
            ("F11".into(), Action::Count),
        ]))
        .unwrap(),
        Default::default(),
    );
    key(&mut c, "Alt+Enter");
    key(&mut c, "F1");
    assert!(draw(&c).contains("F11 counts directory descendants"));
    key(&mut c, "Ctrl+c");
    assert!(c.quit);
    assert!(c.app.context_help.is_none());
}

mod conflict_fixture;

#[test]
fn help_cannot_confirm_a_ready_destructive_preview_or_acknowledged_exit() {
    use terminal_file_manager::operations::{Dialog, Operation, Plan, Queue, Update};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preserved");
    std::fs::write(&path, b"owned").unwrap();
    for operation in [Operation::Delete, Operation::Trash] {
        let mut queue = Queue::new().unwrap();
        queue
            .preview(Plan::paths(operation, vec![path.clone()], None).unwrap())
            .unwrap();
        let plan = loop {
            if let Update::Preview { plan, .. } = queue.next(Duration::from_secs(5)).unwrap() {
                break *plan;
            }
        };
        let mut c = controller(dialog_fixture::dialog(plan));
        key(&mut c, "F1");
        for input in ["Enter", "D", "T", "d", "t"] {
            key(&mut c, input);
        }
        assert!(c.app.operations.outbox.is_empty());
        key(&mut c, "Esc");
        assert!(
            matches!(&c.app.operations.dialog, Some(Dialog::Operation(d)) if d.ready && d.plan.preview.is_some())
        );
        key(&mut c, "Esc");
        assert!(c.app.operations.outbox.iter().all(|intent| !matches!(
            intent,
            terminal_file_manager::operations::Intent::Submit(..)
        )));
    }
    assert_eq!(std::fs::read(path).unwrap(), b"owned");
    assert_incident_help_stays();
}

fn assert_incident_help_stays() {
    use terminal_file_manager::operations::Dialog;
    let mut c = controller(App::new("owned-incident-help".into()));
    c.app.operations.dialog = Some(Dialog::IncidentExit {
        save: false,
        typed: "ACKNOWLEDGE".into(),
    });
    key(&mut c, "F1");
    key(&mut c, "F10");
    assert!(!c.quit);
    key(&mut c, "Esc");
    assert!(
        matches!(&c.app.operations.dialog, Some(Dialog::IncidentExit { typed, .. }) if typed == "ACKNOWLEDGE")
    );
    key(&mut c, "Esc");
    assert!(!c.quit);
}
