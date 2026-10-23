use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::{fs, time::Duration};
use terminal_file_manager::{
    app::App,
    fs::IgnoreContext,
    input::{Action, Bindings, Controller},
    operations::{Decision, Dialog, Intent, Operation, Outcome, execute},
    ui,
};

fn rename_selected(name: std::ffi::OsString, display_only: bool) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join(name);
    fs::write(&source, "native bytes").unwrap();
    let original = terminal_file_manager::fs::inspect(&source);
    let mut controller = controller(root.path());
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.dispatch(Action::Rename, &geometry);
    let Some(Dialog::Operation(dialog)) = &controller.app.operations.dialog else {
        panic!("rename dialog");
    };
    assert!(
        !dialog.text.is_empty(),
        "selected native name must be represented"
    );
    assert_eq!(dialog.plan.operands[0].path, source);
    assert_eq!(dialog.plan.operands[0].identity, original.identity);
    if display_only {
        send(
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        );
        assert!(controller.app.operations.outbox.is_empty());
        let Some(Dialog::Operation(dialog)) = &controller.app.operations.dialog else {
            panic!("explicit replacement required");
        };
        assert!(dialog.error.as_ref().unwrap().contains("replacement"));
    }
    send(
        &mut controller,
        Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
    );
    send(&mut controller, Event::Paste("replacement-雪".into()));
    send(
        &mut controller,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    );
    let Some(Intent::Submit(plan, options)) = controller.app.operations.outbox.pop() else {
        panic!("deliberate rename");
    };
    assert_eq!(plan.operation, Operation::Rename);
    assert_eq!(plan.operands[0].path, source);
    let record = execute(&plan, options, &false.into(), |_| Decision::Skip);
    assert_eq!(record.entries[0].outcome, Outcome::Completed, "{record:?}");
    assert!(!source.exists());
    assert_eq!(
        fs::read(root.path().join("replacement-雪")).unwrap(),
        b"native bytes"
    );
}

fn controller(root: &std::path::Path) -> Controller {
    let mut app = App::new(root.into());
    app.complete_listing(
        0,
        1,
        Ok(
            terminal_file_manager::fs::list(root, &IgnoreContext::default(), &false.into())
                .unwrap(),
        ),
    );
    Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}

fn send(controller: &mut Controller, event: Event) {
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.handle(event, &geometry, Duration::ZERO);
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_rename_requires_a_deliberate_unicode_replacement() {
    use std::os::unix::ffi::OsStringExt;
    rename_selected(std::ffi::OsString::from_vec(b"native-\xff".to_vec()), true);
}

#[test]
fn unicode_rename_keeps_native_selection_and_accepts_a_replacement() {
    rename_selected("native-λ".into(), false);
}
