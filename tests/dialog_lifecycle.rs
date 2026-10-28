use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    color::Theme,
    operations::{Job, JobState, NoticeSeverity, Operation, Plan, Record},
    ui,
};
mod dialog_fixture;

#[test]
fn completed_history_never_paints_into_new_dialog_context_or_controls() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("remaining");
    std::fs::write(&file, b"retained").unwrap();
    for previous in [
        Operation::Delete,
        Operation::Trash,
        Operation::Copy,
        Operation::Move,
    ] {
        for next in [
            Operation::Copy,
            Operation::Move,
            Operation::Mkdir,
            Operation::Trash,
        ] {
            let plan = if next == Operation::Mkdir {
                Plan::mkdir(root.path().into())
            } else {
                Plan::paths(next, vec![file.clone()], Some(root.path().into())).unwrap()
            };
            transitions(dialog_fixture::dialog(plan), previous);
        }
    }
}

fn transitions(mut app: terminal_file_manager::app::App, previous: Operation) {
    let mut dialog = app.operations.dialog.take();
    completed_job(&mut app, previous);
    for level in [ColorLevel::TrueColor, ColorLevel::NoColor] {
        if app.operations.dialog.is_some() {
            dialog = app.operations.dialog.take();
        }
        let theme = Theme::new(level);
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        app.operations.show_history = true;
        terminal
            .draw(|f| {
                ui::render(f, &app, &theme);
            })
            .unwrap();
        app.operations.show_history = false;
        terminal
            .draw(|f| {
                ui::render(f, &app, &theme);
            })
            .unwrap();
        // Test both a live notice and an expired one with different modal rectangles.
        for (width, height, elapsed) in [(100, 40, 0), (60, 24, 6), (120, 50, 0)] {
            app.now += std::time::Duration::from_secs(elapsed);
            if app.operations.dialog.is_some() {
                dialog = app.operations.dialog.take();
            }
            terminal.backend_mut().resize(width, height);
            terminal.resize(Rect::new(0, 0, width, height)).unwrap();
            terminal
                .draw(|f| {
                    ui::render(f, &app, &theme);
                })
                .unwrap();
            // The dialog is supplied below after restoring it for the next frame.
            assert_fresh(&mut terminal, &mut app, &theme, width, height, &mut dialog);
        }
    }
}

fn completed_job(app: &mut terminal_file_manager::app::App, previous: Operation) {
    let mut completed = Plan::mkdir(app.panes[0].path.clone());
    completed.operation = previous;
    app.operations.history.push(Job {
        presentation: None,
        retry_eligible: false,
        id: 1,
        plan: completed,
        options: Default::default(),
        state: JobState::Completed,
        record: Some(Record::default()),
        progress: None,
        started: app.now,
        finished: Some(app.now),
        retry_of: None,
    });
    app.operations.notify(
        "Completed previous operation".into(),
        app.now,
        NoticeSeverity::Success,
    );
}

fn assert_fresh(
    terminal: &mut Terminal<TestBackend>,
    app: &mut terminal_file_manager::app::App,
    theme: &Theme,
    width: u16,
    height: u16,
    dialog: &mut Option<terminal_file_manager::operations::Dialog>,
) {
    app.operations.dialog = dialog.take();
    terminal
        .draw(|f| {
            ui::render(f, app, theme);
        })
        .unwrap();
    assert!(
        !ui::geometry(app, Rect::new(0, 0, width, height))
            .operation_controls
            .iter()
            .any(|c| matches!(c.target, ui::OperationTarget::Job(_)))
    );
    let history = std::mem::take(&mut app.operations.history);
    let mut fresh = Terminal::new(TestBackend::new(width, height)).unwrap();
    fresh
        .draw(|f| {
            ui::render(f, app, theme);
        })
        .unwrap();
    assert_eq!(terminal.backend().buffer(), fresh.backend().buffer());
    app.operations.history = history;
}
