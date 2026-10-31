//! Native listing, measured buffer, keyboard and mouse contracts.
use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::{sync::atomic::AtomicBool, time::Duration};
use terminal_file_manager::{
    app::{App, PatternMode, PatternOperation},
    color::Theme,
    fs::{self, IgnoreContext},
    input::{Action, Bindings, Controller, chord},
    ui::{self, PatternControl},
};

fn loaded() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    for name in ["alpha.txt", "beta.txt", "keep.log"] {
        std::fs::write(root.path().join(name), "owned").unwrap();
    }
    let snapshot = fs::list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let mut app = App::new(root.path().into());
    for id in 0..2 {
        app.complete_listing(id, 1, Ok(snapshot.clone()));
    }
    app.dual = true;
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        ),
    )
}

fn event(c: &mut Controller, event: Event, size: (u16, u16)) {
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, size.0, size.1));
    assert!(c.handle(event, &geometry, Duration::ZERO).is_empty());
}

fn key(c: &mut Controller, input: &str) {
    let (code, modifiers) = chord(input).unwrap();
    event(c, Event::Key(KeyEvent::new(code, modifiers)), (100, 24));
}

fn click(c: &mut Controller, target: PatternControl) {
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, 100, 24));
    let layout = geometry.pattern.unwrap();
    let area = layout
        .fields
        .iter()
        .find(|(_, t)| *t == target)
        .map(|(area, _)| *area)
        .or_else(|| {
            layout
                .shell
                .actions
                .iter()
                .find(|r| r.action.intent == target)
                .map(|r| r.area)
        })
        .unwrap();
    event(
        c,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }),
        (100, 24),
    );
}

fn rendered(c: &Controller, width: u16, height: u16, level: ColorLevel) -> (String, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut geometry = ui::geometry(&c.app, Rect::new(0, 0, width, height));
    terminal
        .draw(|frame| geometry = ui::render(frame, &c.app, &Theme::new(level)))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    (text, geometry)
}

#[test]
fn measured_buttons_fields_counts_and_focus_match_keyboard_and_mouse() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*.txt".into()), (100, 24));
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let (text, geometry) = rendered(&c, 100, 24, level);
        assert!(text.contains("Matches: 2 / 3 selectable"));
        assert!(text.contains("Replace: 0 visible marks -> 2 matches"));
        let layout = geometry.pattern.unwrap();
        for region in &layout.shell.actions {
            assert!(text.contains(&region.action.label));
            assert!(region.area.right() < layout.shell.outer.right());
            assert!(region.area.bottom() < layout.shell.outer.bottom());
        }
        assert!(layout.shell.outer.width < 80);
    }
    click(&mut c, PatternControl::Case);
    assert!(!c.app.pattern.as_ref().unwrap().case_sensitive);
    click(&mut c, PatternControl::Mode(PatternMode::Regex));
    assert_eq!(c.app.pattern.as_ref().unwrap().mode, PatternMode::Regex);
    click(&mut c, PatternControl::Mode(PatternMode::Wildcard));
    click(&mut c, PatternControl::Apply(PatternOperation::Mark));
    assert!(c.app.pattern.is_none());
    assert_eq!(c.app.panes[0].marks.len(), 2);
    assert!(c.app.panes[1].marks.is_empty());
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*.txt".into()), (100, 24));
    for input in ["Tab", "Tab", "Tab", "Tab", "Tab", "Enter"] {
        key(&mut c, input);
    }
    assert!(c.app.panes[0].marks.is_empty());
}

#[test]
fn invalid_and_empty_patterns_disable_apply_and_zero_replace_explicitly_clears() {
    let (_root, mut c) = loaded();
    c.app.panes[0].select_all();
    key(&mut c, "Alt+b");
    for expression in ["", "["] {
        c.app
            .pattern
            .as_mut()
            .unwrap()
            .set_expression(expression.into());
        let (text, geometry) = rendered(&c, 100, 24, ColorLevel::NoColor);
        assert!(text.contains(if expression.is_empty() {
            "non-empty"
        } else {
            "unclosed"
        }));
        assert!(
            geometry
                .pattern
                .unwrap()
                .shell
                .actions
                .iter()
                .filter(|r| matches!(r.action.intent, PatternControl::Apply(_)))
                .all(|r| !r.action.enabled)
        );
    }
    c.app
        .pattern
        .as_mut()
        .unwrap()
        .set_expression("none".into());
    let (text, _) = rendered(&c, 100, 24, ColorLevel::NoColor);
    assert!(text.contains("Replace: 3 visible marks -> 0 matches"));
    click(&mut c, PatternControl::Apply(PatternOperation::Replace));
    assert!(c.app.panes[0].marks.is_empty());
}

#[test]
fn resize_tiny_terminals_and_all_non_form_input_remain_isolated() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*.txt".into()), (100, 24));
    let cursor = c.app.panes[0].cursor.clone();
    for input in ["F5", "F7", "Alt+p", "Alt+b", "Alt+j", "F12", "Down"] {
        key(&mut c, input);
    }
    event(&mut c, Event::FocusLost, (100, 24));
    event(&mut c, Event::FocusGained, (100, 24));
    for width in [0, 1, 7, 20, 40, 80, 120] {
        for height in [0, 1, 4, 8, 16, 24] {
            event(&mut c, Event::Resize(width, height), (width, height));
            let (_, geometry) = rendered(&c, width, height, ColorLevel::NoColor);
            let layout = geometry.pattern.unwrap();
            assert!(layout.shell.outer.right() <= width);
            assert!(layout.shell.outer.bottom() <= height);
            if height <= 8 {
                assert!(
                    layout
                        .shell
                        .actions
                        .iter()
                        .filter(|r| matches!(r.action.intent, PatternControl::Apply(_)))
                        .all(|r| !r.action.enabled)
                );
            }
            event(
                &mut c,
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: 0,
                    row: 0,
                    modifiers: KeyModifiers::NONE,
                }),
                (width, height),
            );
        }
    }
    assert_eq!(c.app.panes[0].cursor, cursor);
    assert_eq!(c.app.active, 0);
    assert!(c.app.panes[0].marks.is_empty());
    assert!(c.app.operations.dialog.is_none());
    assert!(!c.app.diagnostics);
    key(&mut c, "Esc");
    assert!(c.app.pattern.is_none());
}

#[test]
fn enter_twice_marks_repeats_do_not_apply_and_ctrl_c_keeps_quit() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*".into()), (100, 24));
    key(&mut c, "Enter");
    event(
        &mut c,
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )),
        (100, 24),
    );
    assert!(c.app.pattern.is_some());
    key(&mut c, "Enter");
    assert!(c.app.pattern.is_none());
    assert_eq!(c.app.panes[0].marks.len(), 3);
    key(&mut c, "Alt+b");
    key(&mut c, "Ctrl+c");
    assert!(c.quit);
}

#[test]
fn configured_binding_and_mouse_footer_open_the_same_form() {
    let (_root, mut c) = loaded();
    let bindings = Bindings::new(&[("F11".into(), Action::PatternSelection)].into()).unwrap();
    c = Controller::new(c.app, bindings, Default::default());
    key(&mut c, "F11");
    assert!(c.app.pattern.is_some());
    key(&mut c, "Esc");
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, 100, 24));
    let control = geometry
        .footer_controls
        .iter()
        .find(|r| r.action == Action::PatternSelection)
        .unwrap();
    let event = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: control.area.x,
        row: control.area.y,
        modifiers: KeyModifiers::NONE,
    });
    c.handle(event, &geometry, Duration::ZERO);
    assert!(c.app.pattern.is_some());
    assert!(
        c.app
            .help_rows
            .iter()
            .any(|entry| entry.keys.contains("F11") && entry.description.contains("pattern"))
    );
}

#[test]
fn focused_action_uses_cursor_style_and_hidden_controls_cannot_apply() {
    use ratatui::style::Modifier;
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*".into()), (100, 24));
    for _ in 0..4 {
        key(&mut c, "Tab");
    }
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    let mut geometry = ui::geometry(&c.app, Rect::new(0, 0, 100, 24));
    terminal
        .draw(|frame| geometry = ui::render(frame, &c.app, &Theme::new(ColorLevel::NoColor)))
        .unwrap();
    let area = geometry.pattern.unwrap().shell.actions[0].area;
    assert!(
        terminal.backend().buffer()[(area.x, area.y)]
            .modifier
            .contains(Modifier::REVERSED)
    );
    event(&mut c, Event::Resize(20, 4), (20, 4));
    event(
        &mut c,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        (20, 4),
    );
    assert!(c.app.pattern.is_some());
    assert!(c.app.panes[0].marks.is_empty());
}

#[test]
fn release_and_modified_keys_do_not_dismiss_or_apply_the_form() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*".into()), (100, 24));
    for code in [KeyCode::Enter, KeyCode::Esc, KeyCode::Char('c')] {
        let modifiers = if code == KeyCode::Char('c') {
            KeyModifiers::CONTROL
        } else {
            KeyModifiers::NONE
        };
        event(
            &mut c,
            Event::Key(KeyEvent::new_with_kind(
                code,
                modifiers,
                KeyEventKind::Release,
            )),
            (100, 24),
        );
        assert!(c.app.pattern.is_some());
    }
    for modifiers in [
        KeyModifiers::CONTROL,
        KeyModifiers::ALT,
        KeyModifiers::SUPER,
        KeyModifiers::META,
        KeyModifiers::HYPER,
    ] {
        for code in [KeyCode::Enter, KeyCode::Esc, KeyCode::Tab] {
            event(
                &mut c,
                Event::Key(KeyEvent::new(code, modifiers)),
                (100, 24),
            );
            assert!(c.app.pattern.is_some());
        }
    }
    assert!(c.app.panes[0].marks.is_empty());
}

#[path = "pattern_modal/refinement.rs"]
mod refinement;

#[path = "pattern_modal/accessibility.rs"]
mod accessibility;

#[path = "pattern_modal/review.rs"]
mod review;
