use super::*;

#[test]
fn setting_shortcuts_preserve_expression_caret_and_subsequent_typing_and_paste() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    let mut expected = "alpha".to_owned();
    event(&mut c, Event::Paste(expected.clone()), (100, 24));
    for shortcut in ["Alt+x", "Alt+w", "Alt+s"] {
        key(&mut c, shortcut);
        assert_eq!(
            c.app.pattern.as_ref().unwrap().focus,
            PatternControl::Expression,
            "{shortcut}"
        );
        key(&mut c, "w");
        event(&mut c, Event::Paste(".txt".into()), (100, 24));
        expected.push_str("w.txt");
        assert_eq!(c.app.pattern.as_ref().unwrap().expression(), expected);
        for level in [ColorLevel::NoColor, ColorLevel::TrueColor] {
            let (buffer, layout) = frame(&c, 100, 24, level);
            assert!(text_at(&buffer, layout.fields[0].0).contains(&format!("{expected}▏")));
        }
    }
    assert!(c.app.panes[0].marks.is_empty());
}

#[test]
fn setting_shortcuts_preserve_every_noneditor_focus_including_actions() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("alpha".into()), (100, 24));
    for focus in [
        PatternControl::Mode(PatternMode::Wildcard),
        PatternControl::Mode(PatternMode::Regex),
        PatternControl::Case,
        PatternControl::Apply(PatternOperation::Mark),
        PatternControl::Apply(PatternOperation::Unmark),
        PatternControl::Apply(PatternOperation::Replace),
        PatternControl::Cancel,
    ] {
        key(&mut c, "Tab");
        assert_eq!(c.app.pattern.as_ref().unwrap().focus, focus);
        for shortcut in ["x", "s", "w", "Alt+x", "Alt+s", "Alt+w"] {
            key(&mut c, shortcut);
            assert_eq!(c.app.pattern.as_ref().unwrap().focus, focus, "{shortcut}");
        }
    }
    assert!(c.app.panes[0].marks.is_empty());
    key(&mut c, "Esc");
    assert!(c.app.pattern.is_none());
}

#[test]
fn radio_checkbox_and_blank_editor_gap_are_visible_and_measured() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("alpha".into()), (100, 24));
    for level in [ColorLevel::NoColor, ColorLevel::TrueColor] {
        let (buffer, layout) = frame(&c, 100, 24, level);
        let editor = layout.fields[0].0;
        let gap = Rect::new(editor.x, editor.y + 1, editor.width, 1);
        assert_eq!(text_at(&buffer, gap).trim(), "");
        assert_eq!(layout.hit(gap.x, gap.y), None);
        assert_eq!(layout.fields[1].0.y, editor.y + 2);
        assert_eq!(text_at(&buffer, layout.fields[1].0), "(●) w Wildcard");
        assert_eq!(text_at(&buffer, layout.fields[2].0), "( ) x Regex");
        assert_eq!(text_at(&buffer, layout.fields[3].0), "[x] s Case sensitive");
        let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(!text.to_lowercase().contains("native filename"));
        assert!(text.contains("? matches one byte"));
    }
}

#[test]
fn clicked_and_tabbed_settings_keep_deliberate_focus_and_activate_with_space_or_enter() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+b");
    event(&mut c, Event::Paste("alpha".into()), (100, 24));
    click(&mut c, PatternControl::Mode(PatternMode::Wildcard));
    assert_eq!(
        c.app.pattern.as_ref().unwrap().focus,
        PatternControl::Mode(PatternMode::Wildcard)
    );
    key(&mut c, "Tab");
    key(&mut c, "Space");
    assert_eq!(
        c.app.pattern.as_ref().unwrap().focus,
        PatternControl::Mode(PatternMode::Regex)
    );
    assert_eq!(c.app.pattern.as_ref().unwrap().mode, PatternMode::Regex);
    key(&mut c, "Tab");
    for input in ["Space", "Enter"] {
        key(&mut c, input);
        let state = c.app.pattern.as_ref().unwrap();
        assert_eq!(state.focus, PatternControl::Case);
        assert_eq!(state.case_sensitive, input == "Enter");
        let (buffer, layout) = frame(&c, 100, 24, ColorLevel::NoColor);
        assert_eq!(
            text_at(&buffer, layout.fields[3].0),
            if state.case_sensitive {
                "[x] s Case sensitive"
            } else {
                "[ ] s Case sensitive"
            }
        );
        assert!(
            buffer[(layout.fields[3].0.x, layout.fields[3].0.y)]
                .modifier
                .contains(Modifier::REVERSED)
        );
        assert_eq!(text_at(&buffer, layout.fields[1].0), "( ) w Wildcard");
        assert_eq!(text_at(&buffer, layout.fields[2].0), "(●) x Regex");
    }
    assert_case_repeats_ignored(&mut c);
}

fn assert_case_repeats_ignored(c: &mut Controller) {
    click(c, PatternControl::Case);
    assert!(!c.app.pattern.as_ref().unwrap().case_sensitive);
    assert_eq!(c.app.pattern.as_ref().unwrap().focus, PatternControl::Case);
    for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
        event(
            c,
            Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char(' '),
                KeyModifiers::NONE,
                kind,
            )),
            (100, 24),
        );
        assert!(!c.app.pattern.as_ref().unwrap().case_sensitive);
    }
    assert!(c.app.panes[0].marks.is_empty());
}
