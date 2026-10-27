use colored_text::ColorLevel;
use crossterm::event::{
    Event as Input, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use std::sync::mpsc::sync_channel;
use std::time::Duration;
use terminal_file_manager::ui;
use terminal_file_manager::{
    color::{Role, Theme},
    fs::EntryKind,
    input::{Bindings, Controller},
    operations::Decision,
    ui::OperationTarget,
};
mod conflict_fixture;
use conflict_fixture::{draw, edit, fixture};
#[test]
fn unavailable_choices_are_visible_inert_and_skipped_by_arrow_navigation() {
    let (_root, mut app, response) = fixture();
    edit(&mut app, |_, d, a| {
        d.kind = EntryKind::Directory;
        *a = false;
    });
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let (_, g, text) = draw(&c.app, 80, 30, ColorLevel::NoColor);
    assert!(text.contains("Replace disabled") && text.contains("Newer disabled"));
    for code in [KeyCode::Char('r'), KeyCode::Char('n')] {
        let control = g
            .operation_controls
            .iter()
            .find(|b| b.target == OperationTarget::Key(code))
            .unwrap();
        assert!(!control.enabled);
        assert_eq!(g.operation_target(control.area.x, control.area.y), None);
        click(&mut c, &g, control.area);
        key(&mut c, &g, code);
        assert!(response.try_recv().is_err());
    }
    key(&mut c, &g, KeyCode::Down);
    assert_eq!(c.app.operations.conflicts.front().unwrap().choice, 2);
    key(&mut c, &g, KeyCode::Down);
    assert_eq!(c.app.operations.conflicts.front().unwrap().choice, 4);
    key(&mut c, &g, KeyCode::Enter);
    assert!(matches!(response.try_recv().unwrap(), Decision::Cancel));
}
#[test]
fn shortcuts_keep_existing_typed_decisions_and_initial_skip_focus() {
    for code in [
        KeyCode::Enter,
        KeyCode::Char('s'),
        KeyCode::Char('r'),
        KeyCode::Char('k'),
        KeyCode::Char('n'),
        KeyCode::Esc,
    ] {
        let (_root, app, response) = fixture();
        let mut c = Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        );
        let (terminal, g, _) = draw(&c.app, 80, 25, ColorLevel::TrueColor);
        let skip = g
            .operation_controls
            .iter()
            .find(|b| b.target == OperationTarget::Key(KeyCode::Char('s')))
            .unwrap();
        assert_eq!(
            terminal.backend().buffer()[(skip.area.x, skip.area.y)].fg,
            Theme::new(ColorLevel::TrueColor)
                .style(Role::Cursor)
                .fg
                .unwrap()
        );
        key(&mut c, &g, code);
        let reply = response.try_recv().unwrap();
        assert!(match code {
            KeyCode::Enter | KeyCode::Char('s') => matches!(reply, Decision::Skip),
            KeyCode::Char('r') => matches!(reply, Decision::Replace),
            KeyCode::Char('k') => matches!(reply, Decision::KeepBoth),
            KeyCode::Char('n') => matches!(reply, Decision::Newer),
            KeyCode::Esc => matches!(reply, Decision::Cancel),
            _ => false,
        });
        assert!(c.app.operations.outbox.is_empty());
    }
}
#[test]
fn apply_all_mouse_and_keyboard_toggle_and_cancel_stays_job_cancel() {
    let (_root, app, response) = fixture();
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let (_, g, _) = draw(&c.app, 80, 25, ColorLevel::NoColor);
    let toggle = g
        .operation_controls
        .iter()
        .find(|b| b.target == OperationTarget::Key(KeyCode::Char('a')))
        .unwrap();
    click(&mut c, &g, toggle.area);
    let (_, on, _) = draw(&c.app, 80, 25, ColorLevel::NoColor);
    let toggled = on
        .operation_controls
        .iter()
        .find(|b| b.target == OperationTarget::Key(KeyCode::Char('a')))
        .unwrap();
    assert_eq!(toggled.area, toggle.area);
    assert!(
        draw(&c.app, 80, 25, ColorLevel::NoColor)
            .2
            .contains("A Apply to all: On")
    );
    key(&mut c, &g, KeyCode::Char('A'));
    assert!(!c.app.operations.conflicts.front().unwrap().all);
    key(&mut c, &g, KeyCode::Char('a'));
    key(&mut c, &g, KeyCode::Char('k'));
    assert!(
        matches!(response.try_recv().unwrap(),Decision::All(decision) if matches!(*decision,Decision::KeepBoth))
    );
}
#[test]
fn repeated_conflicts_reset_scroll_and_selection_and_stale_replace_stays_blocked() {
    let (_root, app, response) = fixture();
    let event = app.operations.conflicts.front().unwrap().event.clone();
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let (_, g, _) = draw(&c.app, 40, 15, ColorLevel::NoColor);
    assert!(g.operation_scroll_limit > 3);
    c.app.operations.scroll = 3;
    key(&mut c, &g, KeyCode::Char('k'));
    assert!(matches!(response.try_recv().unwrap(), Decision::KeepBoth));
    assert_eq!(c.app.operations.scroll, 0);
    let (reply, response) = sync_channel(1);
    c.complete_operation(terminal_file_manager::operations::Update::Conflict {
        id: 2,
        event,
        reply,
    });
    assert_eq!(c.app.operations.conflicts.front().unwrap().choice, 0);
    let (_, wide, _) = draw(&c.app, 120, 40, ColorLevel::NoColor);
    let old = wide
        .operation_controls
        .iter()
        .find(|b| b.target == OperationTarget::Key(KeyCode::Char('r')))
        .unwrap()
        .area;
    key(&mut c, &wide, KeyCode::Down);
    edit(&mut c.app, |_, d, a| {
        d.kind = EntryKind::Directory;
        *a = false;
    });
    let (_, narrow, _) = draw(&c.app, 40, 15, ColorLevel::NoColor);
    click(&mut c, &wide, old);
    key(&mut c, &narrow, KeyCode::Enter);
    assert!(response.try_recv().is_err());
    key(&mut c, &narrow, KeyCode::Esc);
    assert!(matches!(response.try_recv().unwrap(), Decision::Cancel));
}

fn key(c: &mut Controller, g: &ui::Geometry, code: KeyCode) {
    c.handle(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        g,
        Duration::ZERO,
    );
}
fn click(c: &mut Controller, g: &ui::Geometry, area: Rect) {
    c.handle(
        Input::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }),
        g,
        Duration::ZERO,
    );
}

#[test]
fn newer_only_never_chooses_the_existing_file_or_overwrites_with_older_incoming() {
    use std::time::{Duration, UNIX_EPOCH};
    use terminal_file_manager::operations::{Operation, Outcome, Plan, execute};
    let (root, _, _) = fixture();
    let source = root.path().join("source/conflict.txt");
    let existing = root.path().join("destination/conflict.txt");
    for (incoming_ms, expected) in [(50, Outcome::Skipped), (120, Outcome::Completed)] {
        for (path, ms) in [(&source, incoming_ms), (&existing, 90)] {
            std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .unwrap()
                .set_times(
                    std::fs::FileTimes::new()
                        .set_modified(UNIX_EPOCH + Duration::from_millis(1793038260000 + ms)),
                )
                .unwrap();
        }
        let plan = Plan::paths(
            Operation::Copy,
            vec![source.clone()],
            Some(root.path().join("destination")),
        )
        .unwrap();
        let record = execute(&plan, Default::default(), &false.into(), |_| {
            Decision::Newer
        });
        assert_eq!(record.entries[0].outcome, expected);
        assert_eq!(
            std::fs::metadata(&existing).unwrap().len(),
            if expected == Outcome::Completed {
                22
            } else {
                53
            }
        );
        assert_eq!(std::fs::metadata(&source).unwrap().len(), 22);
    }
}

#[test]
fn tiny_type_conflicts_keep_required_warning_before_apply_scope() {
    let (_root, mut app, _) = fixture();
    edit(&mut app, |_, d, a| {
        d.kind = EntryKind::Directory;
        *a = false;
    });
    let (_, g, text) = draw(&app, 40, 10, ColorLevel::NoColor);
    assert!(text.contains("Replace disabled"), "{text}");
    assert!(!g.operation_enabled(KeyCode::Char('r')));
    assert!(!g.operation_enabled(KeyCode::Char('n')));
}

mod dialog_fixture;
#[test]
fn background_conflict_preserves_form_scroll_until_that_form_closes() {
    let (root, app, _) = fixture();
    let event = app.operations.conflicts.front().unwrap().event.clone();
    let mut paths = Vec::new();
    for index in 0..12 {
        let path = root.path().join(format!("selected-{index}"));
        std::fs::write(&path, b"owned").unwrap();
        paths.push(path);
    }
    let plan = terminal_file_manager::operations::Plan::paths(
        terminal_file_manager::operations::Operation::Copy,
        paths,
        Some(root.path().join("destination")),
    )
    .unwrap();
    let mut app = dialog_fixture::dialog(plan);
    app.operations.scroll = 3;
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let (reply, _) = sync_channel(1);
    c.complete_operation(terminal_file_manager::operations::Update::Conflict {
        id: 2,
        event,
        reply,
    });
    assert_eq!(c.app.operations.scroll, 3);
    let (_, g, _) = draw(&c.app, 40, 15, ColorLevel::NoColor);
    assert!(g.operation_scroll_limit >= 3);
    key(&mut c, &g, KeyCode::Esc);
    assert!(c.app.operations.dialog.is_none());
    assert_eq!(c.app.operations.scroll, 0);
    assert_eq!(c.app.operations.conflicts.front().unwrap().choice, 0);
}

#[test]
fn apply_all_label_is_neutral_and_state_has_its_own_style() {
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let (_root, app, _) = fixture();
        let mut c = Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        );
        for _ in 0..5 {
            for (state, role) in [("Off", Role::Disabled), ("On", Role::Status)] {
                let (terminal, g, text) = draw(&c.app, 100, 40, level);
                assert!(text.contains(&format!("A Apply to all: {state}")), "{text}");
                let control = g
                    .operation_controls
                    .iter()
                    .find(|b| b.target == OperationTarget::Key(KeyCode::Char('a')))
                    .unwrap();
                assert_toggle_style(terminal.backend().buffer(), control, state, role, level);
                click(&mut c, &g, control.area);
            }
            let (_, g, _) = draw(&c.app, 100, 40, level);
            key(&mut c, &g, KeyCode::Right);
        }
    }
}

fn assert_toggle_style(
    b: &ratatui::buffer::Buffer,
    control: &ui::OperationControl,
    state: &str,
    role: Role,
    level: ColorLevel,
) {
    let x = control.area.x + 1 + ui::modal::width(control.label.strip_suffix(state).unwrap());
    assert_eq!(b[(x, control.area.y)].symbol(), &state[..1]);
    assert_eq!(
        b[(x, control.area.y)].fg,
        Theme::new(level).style(role).fg.unwrap_or_default()
    );
    assert_ne!(
        b[(x, control.area.y)].modifier,
        b[(control.area.x + 1, control.area.y)].modifier
    );
    assert_eq!(b[(control.area.right() - 1, control.area.y)].symbol(), "]");

    assert_eq!(b[(control.area.x, control.area.y)].symbol(), "[");
}

#[test]
fn invisible_comparison_blocks_overwrite_keys_enter_and_mouse() {
    for (width, height) in [(40, 10), (80, 8), (120, 8)] {
        let (_root, app, response) = fixture();
        let mut c = Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        );
        let (_, g, _) = draw(&c.app, width, height, ColorLevel::NoColor);
        for code in [KeyCode::Char('r'), KeyCode::Char('n')] {
            assert!(!g.operation_enabled(code), "{width}x{height}: {code:?}");
            if let Some(b) = g
                .operation_controls
                .iter()
                .find(|b| b.target == OperationTarget::Key(code))
            {
                click(&mut c, &g, b.area);
            }
            key(&mut c, &g, code);
        }
        c.app.operations.conflicts.front_mut().unwrap().choice = 1;
        key(&mut c, &g, KeyCode::Enter);
        assert!(response.try_recv().is_err());
        key(&mut c, &g, KeyCode::Esc);
        assert!(matches!(response.try_recv().unwrap(), Decision::Cancel));
    }
}
