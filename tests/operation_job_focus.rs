use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::{fs, sync::mpsc, time::Duration};
use terminal_file_manager::{
    app::App,
    input::{Bindings, Controller},
    operations::{Decision, Event as JobEvent, Plan, Update},
    ui,
};

#[test]
fn opening_a_selected_jobs_conflict_never_targets_another_waiting_job() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), "incoming").unwrap();
    fs::write(root.path().join("target"), "existing").unwrap();
    let app = App::new(root.path().into());
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let mut responses = Vec::new();
    for id in [1, 2] {
        controller.register_operation(
            id,
            Plan::mkdir(root.path().join(format!("directory-{id}"))),
            Default::default(),
            None,
        );
        let (reply, response) = mpsc::sync_channel(1);
        responses.push(response);
        let source = terminal_file_manager::fs::inspect(&root.path().join("source"));
        let destination = terminal_file_manager::fs::inspect(&root.path().join("target"));
        controller.complete_operation(Update::Conflict {
            id,
            event: JobEvent::Conflict {
                source: Box::new(source),
                destination: Box::new(destination),
                newer_available: true,
            },
            reply,
        });
    }
    controller.app.operations.show_history = true;
    assert_eq!(controller.app.operations.cursor, 1);
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 120, 30));
    controller.handle(
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &geometry,
        Duration::ZERO,
    );
    assert_eq!(controller.app.operations.conflicts.front().unwrap().id, 2);
    controller.handle(
        Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
        &geometry,
        Duration::ZERO,
    );
    assert!(matches!(responses[1].try_recv().unwrap(), Decision::Skip));
    assert!(responses[0].try_recv().is_err());
}
