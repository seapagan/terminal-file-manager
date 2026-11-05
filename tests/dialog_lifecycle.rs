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
    let mut elapsed_time = std::time::Duration::ZERO;
    for level in [ColorLevel::TrueColor, ColorLevel::NoColor] {
        app.notifications.notify(
            "Completed previous operation".into(),
            app.now,
            NoticeSeverity::Success,
        );
        if app.operations.dialog.is_some() {
            dialog = app.operations.dialog.take();
        }
        let theme = Theme::new(level);
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        app.operations.show_history = true;
        paint(&mut terminal, &app, &theme);
        app.operations.show_history = false;
        paint(&mut terminal, &app, &theme);
        // Test both a live notice and an expired one with different modal rectangles.
        for (width, height, elapsed, live) in
            [(100, 40, 0, true), (60, 24, 6, false), (120, 50, 0, false)]
        {
            let delta = std::time::Duration::from_secs(elapsed);
            app.now += delta;
            elapsed_time += delta;
            app.notifications.advance(elapsed_time);
            assert_eq!(
                !ui::geometry(&app, Rect::new(0, 0, width, height))
                    .toasts
                    .is_empty(),
                live
            );
            if app.operations.dialog.is_some() {
                dialog = app.operations.dialog.take();
            }
            terminal.backend_mut().resize(width, height);
            terminal.resize(Rect::new(0, 0, width, height)).unwrap();
            paint(&mut terminal, &app, &theme);
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
    app.notifications.notify(
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

fn paint(
    terminal: &mut Terminal<TestBackend>,
    app: &terminal_file_manager::app::App,
    theme: &Theme,
) {
    terminal
        .draw(|f| {
            ui::render(f, app, theme);
        })
        .unwrap();
}
