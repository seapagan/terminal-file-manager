use super::refinement::{frame, text_at};
use super::*;

fn action_event(c: &mut Controller, size: (u16, u16), op: PatternOperation, mouse: bool) {
    let (_, layout) = frame(c, size.0, size.1, ColorLevel::NoColor);
    let region = layout
        .shell
        .actions
        .iter()
        .find(|r| r.action.intent == PatternControl::Apply(op))
        .unwrap();
    let input = if mouse {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: region.area.x + region.area.width / 2,
            row: region.area.y,
            modifiers: KeyModifiers::NONE,
        })
    } else {
        Event::Key(KeyEvent::new(
            KeyCode::Char(match op {
                PatternOperation::Mark => 'm',
                PatternOperation::Unmark => 'u',
                PatternOperation::Replace => 'r',
            }),
            KeyModifiers::ALT,
        ))
    };
    event(c, input, size);
}

#[test]
fn short_scrolling_forms_apply_when_fields_and_decision_context_are_readable() {
    for level in [ColorLevel::NoColor, ColorLevel::TrueColor] {
        for size in [(70, 17), (40, 22)] {
            for op in [
                PatternOperation::Mark,
                PatternOperation::Unmark,
                PatternOperation::Replace,
            ] {
                for mouse in [false, true] {
                    let (_root, mut c) = loaded();
                    if op != PatternOperation::Mark {
                        c.app.panes[0].select_all();
                    }
                    key(&mut c, "Alt+b");
                    event(&mut c, Event::Paste("*.txt".into()), size);
                    let (buffer, layout) = frame(&c, size.0, size.1, level);
                    assert!(layout.scroll_limit > 0);
                    assert_eq!(layout.fields.len(), 4, "{size:?} {layout:?}");
                    assert!(text_at(&buffer, layout.fields[0].0).contains("*.txt▏"));
                    assert_decision_context(&buffer, &layout, c.app.panes[0].marks.len());
                    let region = layout
                        .shell
                        .actions
                        .iter()
                        .find(|r| r.action.intent == PatternControl::Apply(op))
                        .unwrap();
                    assert_eq!(
                        text_at(&buffer, region.area),
                        format!("[{}]", region.action.label)
                    );
                    assert!(region.action.enabled, "{size:?} {op:?}");
                    action_event(&mut c, size, op, mouse);
                    assert!(c.app.pattern.is_none());
                    assert_eq!(
                        c.app.panes[0].marks.len(),
                        match op {
                            PatternOperation::Mark => 2,
                            PatternOperation::Unmark => 1,
                            PatternOperation::Replace => 2,
                        }
                    );
                }
            }
        }
    }
}

fn assert_decision_context(
    buffer: &ratatui::buffer::Buffer,
    layout: &ui::PatternLayout,
    marked: usize,
) {
    let footer = layout.shell.footer;
    let text = (footer.y..footer.bottom())
        .map(|y| {
            text_at(
                buffer,
                Rect {
                    y,
                    height: 1,
                    ..footer
                },
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(text.contains("Matches: 2 / 3 selectable"));
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        compact.contains(&format!("Replace:{marked}visiblemarks->2matches")),
        "{text:?}"
    );
}

#[test]
fn scroll_and_resize_fail_closed_then_restore_each_action() {
    for level in [ColorLevel::NoColor, ColorLevel::TrueColor] {
        for op in [
            PatternOperation::Mark,
            PatternOperation::Unmark,
            PatternOperation::Replace,
        ] {
            let (_root, mut c) = loaded();
            key(&mut c, "Alt+b");
            event(&mut c, Event::Paste("*.txt".into()), (70, 17));
            let (before, layout) = frame(&c, 70, 17, level);
            assert!(text_at(&before, layout.fields[0].0).contains("*.txt▏"));
            event(&mut c, Event::Key(KeyCode::PageDown.into()), (70, 17));
            let (scrolled, hidden) = frame(&c, 70, 17, level);
            assert!(
                !hidden
                    .fields
                    .iter()
                    .any(|(_, t)| *t == PatternControl::Expression)
            );
            assert!(text_at(&scrolled, hidden.shell.footer).contains("Matches:"));
            for mouse in [false, true] {
                action_event(&mut c, (70, 17), op, mouse);
                assert!(c.app.pattern.is_some());
                assert!(c.app.panes[0].marks.is_empty());
            }
            event(&mut c, Event::Key(KeyCode::PageUp.into()), (70, 17));
            let (_, restored) = frame(&c, 70, 17, level);
            assert_eq!(restored.fields, layout.fields);
            assert_eq!(restored.shell.outer, layout.shell.outer);
            event(&mut c, Event::Resize(70, 15), (70, 15));
            for mouse in [false, true] {
                action_event(&mut c, (70, 15), op, mouse);
                assert!(c.app.pattern.is_some());
            }
            event(&mut c, Event::Resize(70, 17), (70, 17));
            action_event(&mut c, (70, 17), op, false);
            assert!(c.app.pattern.is_none());
        }
    }
}

#[test]
fn rendered_labels_define_hits_and_gaps_borders_and_outside_are_inert() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*.txt".into()), (70, 17));
    let (buffer, layout) = frame(&c, 70, 17, ColorLevel::NoColor);
    for (label, target) in [
        (
            "(●) w Wildcard",
            PatternControl::Mode(PatternMode::Wildcard),
        ),
        ("( ) x Regex", PatternControl::Mode(PatternMode::Regex)),
        ("[x] s Case sensitive", PatternControl::Case),
        ("[m Mark]", PatternControl::Apply(PatternOperation::Mark)),
        (
            "[u Unmark]",
            PatternControl::Apply(PatternOperation::Unmark),
        ),
        (
            "[r Replace]",
            PatternControl::Apply(PatternOperation::Replace),
        ),
    ] {
        let (x, y) = find_label(&buffer, label);
        assert_eq!(layout.hit(x + 2, y), Some(target));
        assert_eq!(layout.hit(x - 1, y), None);
        assert_eq!(layout.hit(x + label.chars().count() as u16, y), None);
    }
    for (x, y) in [
        (0, 0),
        (layout.shell.outer.x, layout.shell.outer.y),
        (layout.shell.outer.right() - 1, layout.shell.outer.y + 1),
    ] {
        assert_eq!(layout.hit(x, y), None);
        event(
            &mut c,
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE,
            }),
            (70, 17),
        );
        assert!(c.app.pattern.is_some());
        assert!(c.app.panes[0].marks.is_empty());
    }
}

fn find_label(buffer: &ratatui::buffer::Buffer, label: &str) -> (u16, u16) {
    for y in 0..buffer.area.height {
        let line = text_at(buffer, Rect::new(0, y, buffer.area.width, 1));
        if let Some(byte) = line.find(label) {
            return (line[..byte].chars().count() as u16, y);
        }
    }
    panic!("missing rendered label {label}");
}

#[test]
fn expression_caret_tracks_editing_focus_in_color_and_no_color() {
    for level in [ColorLevel::NoColor, ColorLevel::TrueColor] {
        let (_root, mut c) = loaded();
        key(&mut c, "Alt+b");
        event(&mut c, Event::Paste("alpha".into()), (70, 17));
        let (editing, layout) = frame(&c, 70, 17, level);
        let (x, y) = find_label(&editing, "alpha▏");
        assert!(
            editing[(x + 5, y)]
                .modifier
                .contains(ratatui::style::Modifier::BOLD)
        );
        key(&mut c, "Tab");
        let (focused, _) = frame(&c, 70, 17, level);
        assert!(text_at(&focused, layout.fields[0].0).contains("alpha"));
        assert!(!text_at(&focused, layout.fields[0].0).contains('▏'));
        assert_eq!(
            text_at(&editing, layout.fields[1].0),
            text_at(&focused, layout.fields[1].0)
        );
        let area = layout.fields[1].0;
        assert_ne!(
            editing[(area.x, area.y)].style(),
            focused[(area.x, area.y)].style()
        );
    }
}

#[test]
fn invalid_patterns_never_apply_even_with_visible_buttons_on_short_forms() {
    for expression in ["", "["] {
        let (_root, mut c) = loaded();
        c.app.panes[0].select_all();
        let marks = c.app.panes[0].marks.clone();
        key(&mut c, "Alt+b");
        c.app
            .pattern
            .as_mut()
            .unwrap()
            .set_expression(expression.into());
        let (buffer, layout) = frame(&c, 70, 17, ColorLevel::NoColor);
        assert!(
            text_at(&buffer, layout.shell.footer).contains(if expression.is_empty() {
                "non-empty"
            } else {
                "unclosed"
            })
        );
        for op in [
            PatternOperation::Mark,
            PatternOperation::Unmark,
            PatternOperation::Replace,
        ] {
            for mouse in [false, true] {
                action_event(&mut c, (70, 17), op, mouse);
                assert!(c.app.pattern.is_some());
                assert_eq!(c.app.panes[0].marks, marks);
            }
        }
    }
}
