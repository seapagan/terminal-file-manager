use super::*;
use ratatui::{buffer::Buffer, style::Modifier};

fn frame(
    c: &Controller,
    width: u16,
    height: u16,
    level: ColorLevel,
) -> (Buffer, ui::PatternLayout) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut geometry = ui::geometry(&c.app, Rect::new(0, 0, width, height));
    terminal
        .draw(|f| geometry = ui::render(f, &c.app, &Theme::new(level)))
        .unwrap();
    (
        terminal.backend().buffer().clone(),
        geometry.pattern.unwrap(),
    )
}

fn text_at(buffer: &Buffer, area: Rect) -> String {
    (area.x..area.right())
        .map(|x| buffer[(x, area.y)].symbol())
        .collect()
}

fn assert_control_cells(before: &Buffer, layout: &ui::PatternLayout, width: u16) {
    let wildcard = layout.fields[1].0;
    let regex = layout.fields[2].0;
    assert_eq!(text_at(before, wildcard), "(●) W Wildcard");
    assert_eq!(text_at(before, regex), "( ) X Regex");
    assert!(
        before[(wildcard.x, wildcard.y)]
            .modifier
            .contains(Modifier::BOLD | Modifier::UNDERLINED)
    );
    assert!(
        !before[(wildcard.x, wildcard.y)]
            .modifier
            .contains(Modifier::REVERSED)
    );
    assert!(
        before[(regex.x, regex.y)]
            .modifier
            .contains(Modifier::REVERSED)
    );
    assert!(
        !before[(regex.x, regex.y)]
            .modifier
            .contains(Modifier::UNDERLINED)
    );
    assert_eq!(layout.shell.outer.width, width.min(76) - 2);
    assert_eq!(wildcard.width, 14);
    assert_eq!(regex, Rect::new(wildcard.right() + 2, wildcard.y, 11, 1));
    assert_eq!(
        layout.fields[3].0,
        Rect::new(regex.right() + 2, regex.y, 20, 1)
    );
}

#[test]
fn selected_and_focused_controls_have_distinct_stable_monochrome_cells() {
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        for width in [70, 80, 120] {
            let (_root, mut c) = loaded();
            key(&mut c, "Alt+b");
            event(&mut c, Event::Paste("alpha".into()), (width, 24));
            key(&mut c, "Tab");
            key(&mut c, "Tab");
            let (before, layout) = frame(&c, width, 24, level);
            assert_control_cells(&before, &layout, width);
            let wildcard = layout.fields[1].0;
            let regex = layout.fields[2].0;
            let mut changed = Vec::new();
            for input in ["Enter", "w", "x", "s", "Space", "Tab"] {
                key(&mut c, input);
                let (buffer, after) = frame(&c, width, 24, level);
                assert_eq!(after.fields, layout.fields);
                assert_eq!(after.shell.outer, layout.shell.outer);
                assert_eq!(
                    after
                        .shell
                        .actions
                        .iter()
                        .map(|r| r.area)
                        .collect::<Vec<_>>(),
                    layout
                        .shell
                        .actions
                        .iter()
                        .map(|r| r.area)
                        .collect::<Vec<_>>()
                );
                changed.push(text_at(&buffer, regex));
            }
            assert_eq!(changed[0], "(●) X Regex");
            assert_eq!(changed[1], "( ) X Regex");
            assert_eq!(layout.hit(wildcard.x - 1, wildcard.y), None);
            assert!(
                before[(layout.fields[0].0.x + 9, layout.fields[0].0.y)]
                    .modifier
                    .contains(Modifier::UNDERLINED)
            );
        }
    }
}

#[test]
fn editor_letters_stay_literal_and_alt_controls_are_direct_deliberate_actions() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    for letter in ["w", "x", "s", "m", "u", "r", "?", "*", "."] {
        key(&mut c, letter);
    }
    assert_eq!(c.app.pattern.as_ref().unwrap().expression(), "wxsmur?*.");
    key(&mut c, "Alt+x");
    assert_eq!(c.app.pattern.as_ref().unwrap().mode, PatternMode::Regex);
    key(&mut c, "Alt+w");
    key(&mut c, "Alt+s");
    assert!(!c.app.pattern.as_ref().unwrap().case_sensitive);
    click(&mut c, PatternControl::Expression);
    c.app.pattern.as_mut().unwrap().set_expression("*".into());
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        event(
            &mut c,
            Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char('m'),
                KeyModifiers::ALT,
                kind,
            )),
            (100, 24),
        );
        assert!(c.app.pattern.is_some());
        assert!(c.app.panes[0].marks.is_empty());
    }
    key(&mut c, "Alt+m");
    assert_eq!(c.app.panes[0].marks.len(), 3);
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*".into()), (100, 24));
    key(&mut c, "Alt+u");
    assert!(c.app.panes[0].marks.is_empty());
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("missing".into()), (100, 24));
    c.app.panes[0].select_all();
    key(&mut c, "Alt+r");
    assert!(c.app.panes[0].marks.is_empty());
}

#[test]
fn clipped_context_disables_keyboard_and_mouse_application_and_hover_is_inert() {
    for level in [ColorLevel::NoColor, ColorLevel::TrueColor] {
        for width in [20, 40, 70, 80, 120] {
            for height in [4, 8, 24] {
                assert_clipped_context(width, height, level);
            }
        }
    }
}

fn assert_clipped_context(width: u16, height: u16, level: ColorLevel) {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*".into()), (width, height));
    let (buffer, layout) = frame(&c, width, height, level);
    for region in &layout.shell.actions {
        assert_eq!(
            text_at(&buffer, region.area),
            format!("[{}]", region.action.label)
        );
        if matches!(region.action.intent, PatternControl::Apply(_)) && !region.action.enabled {
            assert_eq!(layout.hit(region.area.x, region.area.y), None);
            event(
                &mut c,
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: region.area.x,
                    row: region.area.y,
                    modifiers: KeyModifiers::NONE,
                }),
                (width, height),
            );
            event(
                &mut c,
                Event::Key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT)),
                (width, height),
            );
            assert!(c.app.pattern.is_some());
            assert!(c.app.panes[0].marks.is_empty());
        }
    }
    for (area, _) in &layout.fields {
        event(
            &mut c,
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Moved,
                column: area.x,
                row: area.y,
                modifiers: KeyModifiers::NONE,
            }),
            (width, height),
        );
    }
    assert_eq!(c.app.pattern.as_ref().unwrap().focus, 0);
    assert!(c.app.panes[0].marks.is_empty());
}

#[test]
fn conflict_arrival_consumes_the_event_before_any_conflict_choice() {
    use terminal_file_manager::operations::{Decision, Event as OperationEvent, Update};
    for code in [
        KeyCode::Char('r'),
        KeyCode::Char('n'),
        KeyCode::Enter,
        KeyCode::Esc,
    ] {
        let (_root, mut c) = loaded();
        key(&mut c, "Alt+b");
        let (reply, response) = std::sync::mpsc::sync_channel(1);
        c.complete_operation(Update::Conflict {
            id: 1,
            event: OperationEvent::Conflict {
                source: Box::new(c.app.panes[0].entries[0].clone()),
                destination: Box::new(c.app.panes[0].entries[1].clone()),
                newer_available: true,
            },
            reply,
        });
        event(
            &mut c,
            Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            (100, 24),
        );
        assert!(c.app.pattern.is_none());
        assert!(
            response.try_recv().is_err(),
            "transition key {code:?} approved a conflict"
        );
        assert_eq!(c.app.operations.conflicts.len(), 1);
        key(&mut c, "r");
        assert!(matches!(response.try_recv().unwrap(), Decision::Replace));
    }
}

#[test]
fn preferred_width_and_enter_twice_mark_without_accidental_application() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    key(&mut c, "Enter");
    assert_eq!(c.app.pattern.as_ref().unwrap().focus, 0);
    event(&mut c, Event::Paste("*".into()), (100, 24));
    for width in [80, 120] {
        let (_, geometry) = rendered(&c, width, 24, ColorLevel::NoColor);
        assert!((70..=76).contains(&geometry.pattern.unwrap().shell.outer.width));
    }
    key(&mut c, "Enter");
    assert!(c.app.panes[0].marks.is_empty());
    key(&mut c, "Enter");
    assert_eq!(c.app.panes[0].marks.len(), 3);
}

#[test]
fn mode_case_and_validation_never_move_visible_control_rectangles() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    for width in [20, 40, 70, 80, 120] {
        c.app
            .pattern
            .as_mut()
            .unwrap()
            .set_mode(PatternMode::Wildcard);
        c.app.pattern.as_mut().unwrap().set_expression("*".into());
        let (_, before) = frame(&c, width, 30, ColorLevel::NoColor);
        for mode in [PatternMode::Regex, PatternMode::Wildcard] {
            c.app.pattern.as_mut().unwrap().set_mode(mode);
            c.app.pattern.as_mut().unwrap().toggle_case();
            let (_, after) = frame(&c, width, 30, ColorLevel::NoColor);
            assert_eq!(before.fields, after.fields, "width {width}, {mode:?}");
            assert_eq!(before.shell.outer, after.shell.outer);
            assert_eq!(
                before
                    .shell
                    .actions
                    .iter()
                    .map(|r| r.area)
                    .collect::<Vec<_>>(),
                after
                    .shell
                    .actions
                    .iter()
                    .map(|r| r.area)
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn priority_transition_consumes_mouse_and_hover() {
    use terminal_file_manager::operations::{Event as OperationEvent, Update};
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Moved,
    ] {
        let (_root, mut c) = loaded();
        key(&mut c, "Alt+b");
        let (reply, response) = std::sync::mpsc::sync_channel(1);
        c.complete_operation(Update::Conflict {
            id: 1,
            event: OperationEvent::Conflict {
                source: Box::new(c.app.panes[0].entries[0].clone()),
                destination: Box::new(c.app.panes[0].entries[1].clone()),
                newer_available: true,
            },
            reply,
        });
        let geometry = ui::geometry(&c.app, Rect::new(0, 0, 100, 24));
        let replace = geometry
            .operation_controls
            .iter()
            .find(|r| r.target == ui::OperationTarget::Key(KeyCode::Char('r')))
            .unwrap();
        event(
            &mut c,
            Event::Mouse(MouseEvent {
                kind,
                column: replace.area.x,
                row: replace.area.y,
                modifiers: KeyModifiers::NONE,
            }),
            (100, 24),
        );
        assert!(response.try_recv().is_err());
        assert_eq!(c.app.operations.conflicts.len(), 1);
        assert!(c.app.pattern.is_none());
    }
}

#[test]
fn priority_transition_consumes_resize_release_and_dialog_approval_but_keeps_quit() {
    use terminal_file_manager::operations::Dialog;
    for input in [
        Event::Resize(40, 16),
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Release,
        )),
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    ] {
        let (_root, mut c) = loaded();
        key(&mut c, "Alt+b");
        c.app.operations.dialog = Some(Dialog::Exit { save: false });
        event(&mut c, input, (40, 16));
        assert!(c.app.pattern.is_none());
        assert!(matches!(c.app.operations.dialog, Some(Dialog::Exit { .. })));
        assert!(!c.quit);
    }
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    c.app.operations.dialog = Some(Dialog::Exit { save: false });
    key(&mut c, "Ctrl+c");
    assert!(c.quit);
    assert!(c.app.pattern.is_none());
}

#[path = "polish.rs"]
mod polish;
