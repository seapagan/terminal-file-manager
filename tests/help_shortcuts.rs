//! The same Help keys select the current topic without capturing editable text.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Action, Bindings, Controller, HelpTopic, chord},
    operations::{Dialog, Operation, Plan},
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
    send(c, KeyEvent::new(code, modifiers));
}

fn send(c: &mut Controller, key: KeyEvent) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
    assert!(c.handle(Event::Key(key), &g, Duration::ZERO).is_empty());
}

#[test]
fn main_help_lists_both_synonyms_once_and_each_opens_the_same_document() {
    for binding in ["?", "F1"] {
        let mut c = controller(App::new("owned-help-keys".into()));
        let entry = c
            .app
            .help_rows
            .iter()
            .find(|e| e.action == Action::Help)
            .unwrap();
        assert_eq!(entry.keys, "? / F1");
        assert_eq!(entry.description, "Help");
        key(&mut c, binding);
        assert!(c.app.help && c.app.context_help.is_none());
        let mut terminal = Terminal::new(TestBackend::new(160, 160)).unwrap();
        terminal
            .draw(|f| {
                ui::render(f, &c.app, &Theme::new(ColorLevel::NoColor));
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("? / F1"));
        assert!(!text.contains("F1 explains"));
        key(&mut c, "Esc");
        assert!(!c.app.help && !c.quit);
    }
}

#[test]
fn both_keys_open_pattern_context_outside_editing_and_restore_the_same_focus() {
    for binding in ["?", "F1"] {
        let mut c = controller(App::new("owned-help-keys".into()));
        key(&mut c, "Alt+b");
        key(&mut c, "Tab");
        let focus = c.app.pattern.as_ref().unwrap().focus;
        assert_ne!(focus, ui::PatternControl::Expression);
        key(&mut c, binding);
        assert_eq!(
            c.app.context_help.as_ref().unwrap().topic,
            HelpTopic::Pattern
        );
        assert!(!c.app.help);
        key(&mut c, "Enter");
        key(&mut c, "Esc");
        assert_eq!(c.app.pattern.as_ref().unwrap().focus, focus);
        assert!(c.app.operations.outbox.is_empty());
        key(&mut c, "Esc");
        assert!(c.app.pattern.is_none());
    }
}

#[test]
fn editable_pattern_keeps_question_marks_with_or_without_shift_and_f1_remains_available() {
    let mut c = controller(App::new("owned-help-keys".into()));
    key(&mut c, "Alt+b");
    for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
        send(&mut c, KeyEvent::new(KeyCode::Char('?'), modifiers));
    }
    assert_eq!(c.app.pattern.as_ref().unwrap().expression(), "??");
    key(&mut c, "F1");
    assert_eq!(
        c.app.context_help.as_ref().unwrap().topic,
        HelpTopic::Pattern
    );
    key(&mut c, "Esc");
    assert_eq!(c.app.pattern.as_ref().unwrap().expression(), "??");
    assert_eq!(
        c.app.pattern.as_ref().unwrap().focus,
        ui::PatternControl::Expression
    );
}

#[test]
fn operation_fields_keep_question_marks_and_f1_retains_their_text() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source");
    std::fs::write(&path, b"owned").unwrap();
    let mut plans = vec![
        Plan::create_file(root.path().into()),
        Plan::mkdir(root.path().into()),
    ];
    for operation in [
        Operation::Copy,
        Operation::Move,
        Operation::Rename,
        Operation::ReadOnly(true),
    ] {
        plans.push(
            Plan::paths(
                operation,
                vec![path.clone()],
                Some(root.path().join("dest")),
            )
            .unwrap(),
        );
    }
    for plan in plans {
        let operation = plan.operation;
        let mut c = controller(dialog_fixture::dialog(plan));
        key(&mut c, "Ctrl+a");
        key(&mut c, "?");
        assert!(c.app.context_help.is_none());
        assert!(matches!(&c.app.operations.dialog, Some(Dialog::Operation(d)) if d.text == "?"));
        key(&mut c, "F1");
        assert_eq!(
            c.app.context_help.as_ref().unwrap().topic,
            HelpTopic::Operation(operation)
        );
        key(&mut c, "Esc");
        assert!(matches!(&c.app.operations.dialog, Some(Dialog::Operation(d)) if d.text == "?"));
        assert!(c.app.operations.outbox.is_empty());
    }
}

#[test]
fn question_help_works_for_read_only_and_safety_dialogs() {
    for binding in ["?", "F1"] {
        let mut c = controller(App::new("owned-help-keys".into()));
        c.app.operations.show_history = true;
        key(&mut c, binding);
        assert_eq!(c.app.context_help.as_ref().unwrap().topic, HelpTopic::Jobs);
        key(&mut c, "Esc");
        assert!(c.app.operations.show_history);
        c.app.operations.show_history = false;
        key(&mut c, "Alt+Enter");
        key(&mut c, binding);
        assert_eq!(
            c.app.context_help.as_ref().unwrap().topic,
            HelpTopic::Metadata
        );
        key(&mut c, "Esc");
        assert!(c.app.popup);
        c.app.popup = false;
        c.app.operations.dialog = Some(Dialog::Exit { save: true });
        key(&mut c, binding);
        assert_eq!(c.app.context_help.as_ref().unwrap().topic, HelpTopic::Exit);
        key(&mut c, "Esc");
        assert!(matches!(
            c.app.operations.dialog,
            Some(Dialog::Exit { save: true })
        ));
        assert!(!c.quit && c.app.operations.outbox.is_empty());
    }
}

#[test]
fn incident_editor_retains_question_and_help_cannot_acknowledge_or_exit() {
    let mut c = controller(App::new("owned-help-keys".into()));
    c.app.operations.dialog = Some(Dialog::IncidentExit {
        save: true,
        typed: String::new(),
    });
    key(&mut c, "?");
    assert!(c.app.context_help.is_none());
    key(&mut c, "F1");
    assert_eq!(
        c.app.context_help.as_ref().unwrap().topic,
        HelpTopic::Incident
    );
    key(&mut c, "F10");
    key(&mut c, "Esc");
    assert!(
        matches!(&c.app.operations.dialog, Some(Dialog::IncidentExit { typed, .. }) if typed == "?")
    );
    assert!(!c.quit && !c.save_session);
}

#[test]
fn help_function_keys_ignore_repeats_releases_and_modifiers() {
    let mut c = controller(App::new("owned-help-keys".into()));
    key(&mut c, "Alt+b");
    key(&mut c, "Tab");
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        send(
            &mut c,
            KeyEvent::new_with_kind(KeyCode::F(1), KeyModifiers::NONE, kind),
        );
        send(
            &mut c,
            KeyEvent::new_with_kind(KeyCode::Char('?'), KeyModifiers::NONE, kind),
        );
    }
    for binding in ["Alt+F1", "Ctrl+F1", "Shift+F1", "Alt+?"] {
        key(&mut c, binding);
    }
    assert!(c.app.context_help.is_none());
    assert!(c.app.pattern.is_some());
    assert!(c.app.operations.outbox.is_empty());
}

mod dialog_fixture;

#[test]
fn question_help_cannot_authorise_destructive_dialogs() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preserved");
    std::fs::write(&path, b"owned").unwrap();
    for operation in [Operation::Trash, Operation::Delete] {
        let plan = Plan::paths(operation, vec![path.clone()], None).unwrap();
        let mut c = controller(dialog_fixture::dialog(plan));
        key(&mut c, "?");
        assert_eq!(
            c.app.context_help.as_ref().unwrap().topic,
            HelpTopic::Operation(operation)
        );
        for binding in ["T", "D", "Enter"] {
            key(&mut c, binding);
        }
        assert!(c.app.operations.outbox.is_empty());
        key(&mut c, "Esc");
        assert!(c.app.operations.dialog.is_some());
        key(&mut c, "Esc");
        assert!(c.app.operations.dialog.is_none());
    }
    assert_eq!(std::fs::read(path).unwrap(), b"owned");
}

#[test]
fn configured_main_help_override_keeps_its_effective_action_and_label() {
    let bindings = Bindings::new(&[("F1".into(), Action::Jobs)].into()).unwrap();
    let mut c = Controller::new(
        App::new("owned-help-keys".into()),
        bindings,
        Default::default(),
    );
    let entry = c
        .app
        .help_rows
        .iter()
        .find(|e| e.action == Action::Help)
        .unwrap();
    assert_eq!(entry.keys, "?");
    key(&mut c, "F1");
    assert!(c.app.operations.show_history && !c.app.help);
}
