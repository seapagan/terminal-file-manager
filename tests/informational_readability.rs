//! Meaningful status and unavailability explanations retain normal contrast.
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, style::Modifier};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Bindings, Controller},
    operations::{Dialog, JobState, Operation, Plan},
    ui,
};
mod conflict_fixture;
mod dialog_fixture;

fn assert_readable(app: &App, prefix: &str, role: Role) {
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::TrueColor,
    ] {
        let theme = Theme::new(level);
        let mut terminal = Terminal::new(TestBackend::new(160, 100)).unwrap();
        terminal
            .draw(|f| {
                ui::render(f, app, &theme);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let (x, y) = (0..100)
            .find_map(|y| {
                let text: String = (0..160).map(|x| buffer[(x, y)].symbol()).collect();
                text.find(prefix).map(|x| (x as u16, y))
            })
            .unwrap_or_else(|| panic!("missing {prefix}"));
        for offset in 0..prefix.len() as u16 {
            let cell = &buffer[(x + offset, y)];
            assert!(!cell.modifier.contains(Modifier::DIM), "{prefix}: {cell:?}");
            assert_eq!(cell.fg, theme.style(role).fg.unwrap_or_default());
        }
    }
}

#[test]
fn empty_search_and_pending_preview_explain_their_state_without_dimming() {
    let mut app = App::new("owned-readability".into());
    app.help = true;
    app.help_picker = Some(Default::default());
    app.help_picker.as_mut().unwrap().query = "zzzzzzzz".into();
    assert_readable(&app, "No matching commands", Role::Value);
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preserved");
    std::fs::write(&path, b"owned").unwrap();
    let plan = Plan::paths(Operation::Delete, vec![path.clone()], None).unwrap();
    let mut app = dialog_fixture::dialog(plan);
    if let Some(Dialog::Operation(dialog)) = &mut app.operations.dialog {
        dialog.ready = false;
    }
    assert_readable(&app, "Counting recursively", Role::Label);
    assert_eq!(std::fs::read(path).unwrap(), b"owned");
}

#[test]
fn unavailable_job_actions_keep_their_explanation_readable() {
    let mut c = Controller::new(
        App::new("owned-readability".into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    c.register_operation(
        1,
        Plan::mkdir("owned-readability/new".into()),
        Default::default(),
        None,
    );
    c.app.operations.show_history = true;
    c.app.operations.jobs_evidence = true;
    assert_readable(&c.app, "Retry and reconciliation wait", Role::Value);
    c.app.operations.history[0].state = JobState::Completed;
    assert_readable(&c.app, "Retry unavailable", Role::Value);
    assert!(c.app.operations.outbox.is_empty());
}

#[test]
fn unavailable_conflict_action_keeps_its_reason_readable_and_button_disabled() {
    let (_root, mut app, response) = conflict_fixture::fixture();
    conflict_fixture::edit(&mut app, |_, destination, available| {
        destination.modified = None;
        *available = false;
    });
    assert_readable(&app, "Newer disabled: time unavailable.", Role::Value);
    let (_, geometry, _) = conflict_fixture::draw(&app, 80, 30, ColorLevel::NoColor);
    let newer = geometry
        .operation_controls
        .iter()
        .find(|c| c.label == "n Newer")
        .unwrap();
    assert!(!newer.enabled);
    assert_eq!(geometry.operation_target(newer.area.x, newer.area.y), None);
    assert!(response.try_recv().is_err());
}
