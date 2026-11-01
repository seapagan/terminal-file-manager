use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use terminal_file_manager::{
    input::{Bindings, Controller},
    operations::Decision,
    ui::{self, OperationTarget},
};
mod conflict_fixture;
use conflict_fixture::{draw, fixture};

fn key(c: &mut Controller, g: &ui::Geometry, code: KeyCode) {
    c.handle(
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        g,
        std::time::Duration::ZERO,
    );
}

#[test]
fn all_modifier_wraps_the_actual_shortcut_enter_or_mouse_decision() {
    for all in [false, true] {
        for choice in 0..5 {
            for route in 0..3 {
                let (_root, app, response) = fixture();
                let mut c = Controller::new(
                    app,
                    Bindings::new(&Default::default()).unwrap(),
                    Default::default(),
                );
                let (_, g, _) = draw(&c.app, 120, 40, ColorLevel::NoColor);
                key(&mut c, &g, KeyCode::Char('a'));
                if !all {
                    key(&mut c, &g, KeyCode::Char('a'));
                }
                let text = draw(&c.app, 120, 40, ColorLevel::NoColor).2;
                assert!(text.contains(if all {
                    "a Apply to all: On"
                } else {
                    "a Apply to all: Off"
                }));
                submit(&mut c, choice, route);
                assert_decision(response.try_recv().unwrap(), choice, all);
            }
        }
    }
}

fn submit(c: &mut Controller, choice: usize, route: usize) {
    let codes = [
        KeyCode::Char('s'),
        KeyCode::Char('r'),
        KeyCode::Char('k'),
        KeyCode::Char('n'),
        KeyCode::Esc,
    ];
    let (_, g, _) = draw(&c.app, 120, 40, ColorLevel::NoColor);
    if route == 0 {
        key(c, &g, KeyCode::Right);
        key(c, &g, codes[choice]);
    } else if route == 1 {
        for _ in 0..choice {
            key(c, &g, KeyCode::Right);
        }
        key(c, &g, KeyCode::Enter);
    } else {
        let control = g
            .operation_controls
            .iter()
            .find(|b| b.target == OperationTarget::Key(codes[choice]))
            .unwrap();
        c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: control.area.x,
                row: control.area.y,
                modifiers: KeyModifiers::NONE,
            }),
            &g,
            std::time::Duration::ZERO,
        );
    }
}

fn assert_decision(mut decision: Decision, choice: usize, all: bool) {
    if all && choice != 4 {
        let Decision::All(inner) = decision else {
            panic!("missing All modifier");
        };
        decision = *inner;
    }
    assert!(matches!(
        (choice, decision),
        (0, Decision::Skip)
            | (1, Decision::Replace)
            | (2, Decision::KeepBoth)
            | (3, Decision::Newer)
            | (4, Decision::Cancel)
    ));
}

#[test]
fn all_never_enables_disabled_choices_and_new_prompts_start_safe() {
    let (_root, mut app, response) = fixture();
    conflict_fixture::edit(&mut app, |_, d, available| {
        d.kind = terminal_file_manager::fs::EntryKind::Directory;
        *available = false;
    });
    let event = app.operations.conflicts.front().unwrap().event.clone();
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let (_, g, _) = draw(&c.app, 120, 40, ColorLevel::NoColor);
    key(&mut c, &g, KeyCode::Char('a'));
    for code in [KeyCode::Char('R'), KeyCode::Char('N')] {
        key(&mut c, &g, code);
    }
    assert!(response.try_recv().is_err());
    key(&mut c, &g, KeyCode::Enter);
    assert_decision(response.try_recv().unwrap(), 0, true);
    let (reply, _receiver) = std::sync::mpsc::sync_channel(1);
    c.complete_operation(terminal_file_manager::operations::Update::Conflict {
        id: 2,
        event,
        reply,
    });
    assert_eq!(c.app.operations.conflicts.front().unwrap().choice, 0);
    assert!(!c.app.operations.conflicts.front().unwrap().all);
    assert!(
        draw(&c.app, 120, 40, ColorLevel::NoColor)
            .2
            .contains("a Apply to all: Off")
    );
}

#[test]
fn normal_context_allows_overwrites_and_tiny_default_enter_still_skips() {
    for (width, height) in [(40, 40), (80, 25), (120, 25)] {
        let (_root, app, _) = fixture();
        let (_, g, _) = draw(&app, width, height, ColorLevel::NoColor);
        assert!(g.operation_enabled(KeyCode::Char('r')), "{width}x{height}");
        assert!(g.operation_enabled(KeyCode::Char('n')));
    }
    let (_root, app, response) = fixture();
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let (_, g, _) = draw(&c.app, 80, 8, ColorLevel::NoColor);
    key(&mut c, &g, KeyCode::Enter);
    assert_decision(response.try_recv().unwrap(), 0, false);
}
