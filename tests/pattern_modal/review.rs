use super::*;

#[test]
fn overlong_paste_is_atomic_and_blocks_stale_matcher_until_backspace() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*".into()), (100, 24));
    event(
        &mut c,
        Event::Paste(format!("{}[", "a".repeat(4095))),
        (100, 24),
    );
    assert_eq!(c.app.pattern.as_ref().unwrap().expression(), "*");
    let (text, _) = rendered(&c, 100, 24, ColorLevel::NoColor);
    assert!(text.contains("4096"));
    for input in ["Alt+m", "Alt+u", "Alt+r", "Enter", "Enter"] {
        key(&mut c, input);
        assert!(c.app.pattern.is_some());
        assert!(c.app.panes[0].marks.is_empty());
    }
    key(&mut c, "Backspace");
    event(&mut c, Event::Paste("*".into()), (100, 24));
    key(&mut c, "Enter");
    key(&mut c, "Enter");
    assert!(c.app.pattern.is_none());
    assert_eq!(c.app.panes[0].marks.len(), 3);
}

#[test]
fn invalid_enter_reports_reason_and_preserves_expression_focus() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("[".into()), (100, 24));
    key(&mut c, "Enter");
    assert!(c.app.status.contains("unclosed"), "{}", c.app.status);
    assert!(c.app.panes[0].marks.is_empty());
}

#[test]
fn multibyte_and_large_paste_rejections_block_all_apply_routes_and_settings() {
    for payload in ["é".repeat(2048), format!("{}[", "a".repeat(100_000))] {
        let (_root, mut c) = loaded();
        c.app.panes[0].select_all();
        let marks = c.app.panes[0].marks.clone();
        key(&mut c, "Alt+b");
        event(&mut c, Event::Paste("*".into()), (100, 24));
        event(&mut c, Event::Paste(payload), (100, 24));
        assert_eq!(c.app.pattern.as_ref().unwrap().expression(), "*");
        for input in ["a", "a", "Alt+x", "Alt+s", "Alt+w"] {
            key(&mut c, input);
        }
        assert_eq!(c.app.pattern.as_ref().unwrap().expression(), "*");
        click(&mut c, PatternControl::Case);
        for input in ["m", "u", "r", "Alt+m", "Alt+u", "Alt+r", "Enter"] {
            key(&mut c, input);
            assert!(c.app.pattern.is_some());
            assert_eq!(c.app.panes[0].marks, marks);
        }
        for op in [
            PatternOperation::Mark,
            PatternOperation::Unmark,
            PatternOperation::Replace,
        ] {
            click(&mut c, PatternControl::Apply(op));
            assert!(c.app.pattern.is_some());
            assert_eq!(c.app.panes[0].marks, marks);
        }
        let (text, _) = rendered(&c, 100, 24, ColorLevel::NoColor);
        assert!(text.contains("Backspace to correct"));
        click(&mut c, PatternControl::Expression);
        key(&mut c, "Backspace");
        event(&mut c, Event::Paste("*.txt".into()), (100, 24));
        key(&mut c, "Alt+r");
        assert!(c.app.pattern.is_none());
        assert_eq!(c.app.panes[0].marks.len(), 2);
    }
}

#[test]
fn repeated_typing_at_byte_limit_never_reuses_valid_prefix() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("?".repeat(4094)), (100, 24));
    key(&mut c, "é");
    let accepted = c.app.pattern.as_ref().unwrap().expression().to_owned();
    assert_eq!(accepted.len(), 4096);
    for input in ["x", "x", "é", "Alt+m"] {
        key(&mut c, input);
    }
    assert_eq!(c.app.pattern.as_ref().unwrap().expression(), accepted);
    assert!(
        c.app
            .pattern
            .as_ref()
            .unwrap()
            .preview(&c.app.panes[0])
            .is_err()
    );
    key(&mut c, "Backspace");
    assert_eq!(c.app.pattern.as_ref().unwrap().expression().len(), 4094);
    assert!(
        c.app
            .pattern
            .as_ref()
            .unwrap()
            .preview(&c.app.panes[0])
            .is_ok()
    );
}

#[test]
fn clipped_enter_reports_geometry_and_resize_discards_ineligible_action_focus() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*".into()), (100, 24));
    key(&mut c, "Enter");
    assert_eq!(
        c.app.pattern.as_ref().unwrap().focus,
        PatternControl::Apply(PatternOperation::Mark)
    );
    event(&mut c, Event::Resize(70, 15), (70, 15));
    assert_eq!(
        c.app.pattern.as_ref().unwrap().focus,
        PatternControl::Expression
    );
    event(&mut c, Event::Key(KeyCode::Enter.into()), (70, 15));
    assert!(c.app.status.contains("enlarge terminal"));
    let (text, _) = rendered(&c, 70, 15, ColorLevel::NoColor);
    assert!(text.contains("scroll up or enlarge terminal"));
    assert!(c.app.panes[0].marks.is_empty());
    event(&mut c, Event::Resize(100, 24), (100, 24));
    key(&mut c, "Enter");
    assert!(c.app.pattern.is_some());
    key(&mut c, "Enter");
    assert_eq!(c.app.panes[0].marks.len(), 3);
}

#[test]
fn clicked_settings_explain_plain_action_shortcuts_and_keep_their_contract() {
    for target in [
        PatternControl::Case,
        PatternControl::Mode(PatternMode::Wildcard),
    ] {
        let (_root, mut c) = loaded();
        key(&mut c, "Alt+b");
        event(&mut c, Event::Paste("*".into()), (100, 24));
        click(&mut c, target);
        assert_eq!(c.app.pattern.as_ref().unwrap().focus, target);
        let (text, _) = rendered(&c, 100, 24, ColorLevel::NoColor);
        assert!(text.contains("Outside input: M/U/R apply"));
        key(&mut c, "m");
        assert!(c.app.pattern.is_none());
        assert_eq!(c.app.panes[0].marks.len(), 3);
    }
}

#[test]
fn wheel_and_page_scrolling_share_bounds_and_reconcile_hidden_focus() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("*".into()), (70, 17));
    for wheel in [false, true] {
        c.app.pattern.as_mut().unwrap().scroll = 0;
        for down in [true, false] {
            assert_scroll_route(&mut c, wheel, down);
        }
    }
    event(
        &mut c,
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::PageDown,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )),
        (70, 17),
    );
    assert_eq!(c.app.pattern.as_ref().unwrap().scroll, 0);
    event(&mut c, Event::Key(KeyCode::PageDown.into()), (70, 17));
    event(&mut c, Event::Paste("oops".into()), (70, 17));
    assert_eq!(c.app.pattern.as_ref().unwrap().expression(), "*");
    event(&mut c, Event::Resize(100, 24), (100, 24));
    event(&mut c, Event::Key(KeyCode::PageUp.into()), (100, 24));
    assert_eq!(c.app.pattern.as_ref().unwrap().scroll, 0);
}

fn assert_scroll_route(c: &mut Controller, wheel: bool, down: bool) {
    for _ in 0..8 {
        let layout = ui::geometry(&c.app, Rect::new(0, 0, 70, 17))
            .pattern
            .unwrap();
        let input = if wheel {
            Event::Mouse(MouseEvent {
                kind: if down {
                    MouseEventKind::ScrollDown
                } else {
                    MouseEventKind::ScrollUp
                },
                column: layout.shell.body.x,
                row: layout.shell.body.y,
                modifiers: KeyModifiers::NONE,
            })
        } else {
            Event::Key(
                if down {
                    KeyCode::PageDown
                } else {
                    KeyCode::PageUp
                }
                .into(),
            )
        };
        let before = c.app.pattern.as_ref().unwrap().scroll;
        event(c, input, (70, 17));
        let state = c.app.pattern.as_ref().unwrap();
        let delta = if wheel { 1 } else { 3 };
        assert_eq!(
            state.scroll,
            if down {
                before.saturating_add(delta).min(layout.scroll_limit)
            } else {
                before.saturating_sub(delta)
            }
        );
        assert!(c.app.panes[0].marks.is_empty());
    }
    let limit = ui::geometry(&c.app, Rect::new(0, 0, 70, 17))
        .pattern
        .unwrap()
        .scroll_limit;
    assert_eq!(
        c.app.pattern.as_ref().unwrap().scroll,
        if down { limit } else { 0 }
    );
}
