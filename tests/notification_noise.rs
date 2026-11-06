//! Routine feedback preserves existing toast/history evidence and operation guards.
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    fs::IgnoreContext,
    input::{Action, Bindings, Controller},
    notifications::Severity,
    operations::{Dialog, Record, Update},
    ui,
};

fn loaded() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), "owned").unwrap();
    let mut app = App::new(root.path().into());
    let snapshot =
        terminal_file_manager::fs::list(root.path(), &IgnoreContext::isolated(), &false.into())
            .unwrap();
    app.complete_listing(0, 1, Ok(snapshot));
    app.notify(Severity::Error, "Retained failure");
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        ),
    )
}
fn dispatch(c: &mut Controller, action: Action) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 100, 30));
    c.dispatch(action, &g);
}
fn key(c: &mut Controller, code: KeyCode) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 100, 30));
    c.handle(
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        &g,
        Duration::ZERO,
    );
}
fn assert_retained(c: &Controller) {
    assert_eq!(c.app.notifications.history.len(), 1);
    assert_eq!(c.app.notifications.history[0].text, "Retained failure");
    assert_eq!(c.app.notifications.history[0].occurrences, 1);
    assert_eq!(c.app.notifications.visible(), [0]);
}

#[test]
fn unsupported_actions_and_ineligible_counts_only_change_status() {
    let (_root, mut c) = loaded();
    for action in [
        Action::Viewer,
        Action::Editor,
        Action::Path,
        Action::Zoxide,
        Action::OppositeZoxide,
        Action::ExitDirectory,
        Action::Enter,
        Action::Count,
    ] {
        dispatch(&mut c, action);
        assert_retained(&c);
        assert!(!c.app.status.is_empty());
    }
}

#[test]
fn create_validation_stays_in_the_dialog_without_notifying() {
    let (_root, mut c) = loaded();
    dispatch(&mut c, Action::CreateFile);
    key(&mut c, KeyCode::Enter);
    let Some(Dialog::Operation(dialog)) = &c.app.operations.dialog else {
        panic!("dialog closed")
    };
    assert!(dialog.error.is_some());
    assert!(c.app.operations.outbox.is_empty());
    assert_retained(&c);
}

#[test]
fn invalid_rename_stays_in_the_dialog_without_notifying() {
    let (_root, mut c) = loaded();
    dispatch(&mut c, Action::Rename);
    let Some(Dialog::Operation(dialog)) = &mut c.app.operations.dialog else {
        panic!("no dialog")
    };
    dialog.text = "../elsewhere".into();
    key(&mut c, KeyCode::Enter);
    let Some(Dialog::Operation(dialog)) = &c.app.operations.dialog else {
        panic!("dialog closed")
    };
    assert!(dialog.error.is_some());
    assert!(c.app.operations.outbox.is_empty());
    assert_retained(&c);
}

#[test]
fn pending_destructive_preview_disables_approval_without_notifying() {
    let (_root, mut c) = loaded();
    dispatch(&mut c, Action::Delete);
    c.app.operations.outbox.clear();
    key(&mut c, KeyCode::Char('D'));
    let Some(Dialog::Operation(dialog)) = &c.app.operations.dialog else {
        panic!("dialog closed")
    };
    assert!(!dialog.ready);
    assert!(c.app.operations.outbox.is_empty());
    assert_retained(&c);
}

#[test]
fn failed_preview_keeps_error_visible_and_retained_without_a_job() {
    let (_root, mut c) = loaded();
    dispatch(&mut c, Action::Delete);
    let Some(Dialog::Operation(dialog)) = &mut c.app.operations.dialog else {
        panic!("no dialog")
    };
    dialog.preview_id = Some(42);
    c.complete_operation(Update::Finished {
        id: 42,
        record: Record::default(),
    });
    let Some(Dialog::Operation(dialog)) = &c.app.operations.dialog else {
        panic!("dialog closed")
    };
    assert!(!dialog.ready);
    assert!(dialog.error.is_some());
    assert_eq!(c.app.notifications.history.len(), 2);
    assert_eq!(c.app.notifications.history[1].severity, Severity::Error);
    assert_eq!(c.app.notifications.history[1].job, None);
    assert!(c.app.operations.history.is_empty());
}

#[test]
fn empty_selection_only_changes_status() {
    let (_root, mut c) = loaded();
    c.app.panes[0].set_filter("no matching entry".into());
    dispatch(&mut c, Action::Rename);
    assert_retained(&c);
    assert!(c.app.operations.dialog.is_none());
    assert!(c.app.operations.outbox.is_empty());
}

#[test]
fn rename_with_multiple_marks_only_changes_status() {
    let (root, mut c) = loaded();
    std::fs::write(root.path().join("second"), "owned").unwrap();
    let request = c.app.request_refresh(0).unwrap();
    let snapshot =
        terminal_file_manager::fs::list(root.path(), &IgnoreContext::isolated(), &false.into())
            .unwrap();
    c.app.complete_listing(0, request.generation, Ok(snapshot));
    c.app.panes[0].select_all();
    dispatch(&mut c, Action::Rename);
    assert_retained(&c);
    assert!(c.app.operations.dialog.is_none());
    assert!(c.app.operations.outbox.is_empty());
}

#[test]
fn disabled_conflict_choices_keep_the_prompt_without_notifying() {
    use terminal_file_manager::operations::{Event as JobEvent, PendingConflict};
    let (root, mut c) = loaded();
    let (reply, response) = std::sync::mpsc::sync_channel(1);
    c.app.operations.conflicts.push_back(PendingConflict {
        id: 7,
        choice: 0,
        all: false,
        reply,
        event: JobEvent::Conflict {
            source: Box::new(terminal_file_manager::fs::inspect(
                &root.path().join("file"),
            )),
            destination: Box::new(terminal_file_manager::fs::inspect(root.path())),
            newer_available: false,
        },
    });
    for code in [KeyCode::Char('n'), KeyCode::Char('r')] {
        key(&mut c, code);
        assert_retained(&c);
        assert_eq!(c.app.operations.conflicts.len(), 1);
        assert!(response.try_recv().is_err());
    }
}

#[test]
fn oversized_paste_keeps_form_contents_without_notifying() {
    let (_root, mut c) = loaded();
    dispatch(&mut c, Action::Rename);
    let g = ui::geometry(&c.app, Rect::new(0, 0, 100, 30));
    c.handle(Event::Paste("x".repeat(65_537)), &g, Duration::ZERO);
    let Some(Dialog::Operation(dialog)) = &c.app.operations.dialog else {
        panic!("dialog closed")
    };
    assert_eq!(dialog.text, "file");
    assert!(dialog.error.is_none());
    assert!(c.app.status.contains("paste was not added"));
    assert_retained(&c);
}

#[test]
fn pending_listing_waits_without_notifying_or_opening_an_operation() {
    let (_root, mut c) = loaded();
    c.app.request_listing(0, c.app.panes[0].path.clone());
    dispatch(&mut c, Action::Rename);
    assert_retained(&c);
    assert!(c.app.operations.dialog.is_none());
    assert!(c.app.operations.outbox.is_empty());
}
