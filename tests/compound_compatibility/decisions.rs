use super::*;
use crossterm::event::{KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use terminal_file_manager::ui::CheckIntent;

const COMBINED: &str = "sidebar = true\nmouse = false\ncompound_extensions = ['.tar.gz', '.custom.js']\ncompound_extensions_extend = ['.tar.gz', '.tar.xz', '.tar.xz', '.user.js']";

#[test]
fn combined_choices_are_independent_and_cleanup_follows_the_selected_base() {
    for defaults in [false, true] {
        for remove in [false, true] {
            let mut c = controller(COMBINED, "0.1.0");
            let before = c.preferences.effective_compound_extensions();
            if defaults {
                key(&mut c, KeyCode::Char('d'));
            }
            if remove {
                key(&mut c, KeyCode::Char('r'));
            }
            key(&mut c, KeyCode::Enter);
            assert!(c.app.compound_extension_review.is_none());
            let expected = if defaults {
                None
            } else {
                Some(vec![".tar.gz".into(), ".custom.js".into()])
            };
            assert_eq!(c.preferences.compound_extensions, expected);
            let additions = match (defaults, remove) {
                (_, false) => vec![".tar.gz", ".tar.xz", ".tar.xz", ".user.js"],
                (false, true) => vec![".tar.xz", ".tar.xz", ".user.js"],
                (true, true) => vec![".user.js"],
            };
            assert_eq!(c.preferences.compound_extensions_extend, additions);
            if !defaults {
                assert_eq!(c.preferences.effective_compound_extensions(), before);
            }
            assert!(c.preferences.sidebar);
            assert!(!c.preferences.mouse);
            let text = toml::to_string(&c.preferences).unwrap();
            let reloaded = Settings::parse(&text).unwrap();
            assert_eq!(
                reloaded.compound_extensions,
                c.preferences.compound_extensions
            );
            assert_eq!(reloaded.compound_extensions_extend, additions);
        }
    }
}

#[test]
fn excluded_default_additions_are_not_removable_until_defaults_are_chosen() {
    let mut c = controller(
        "compound_extensions = []\ncompound_extensions_extend = ['.tar.gz', '.user.js']",
        "0.1.0",
    );
    key(&mut c, KeyCode::Char('r'));
    assert!(
        !c.app
            .compound_extension_review
            .as_ref()
            .unwrap()
            .remove_redundant
    );
    let (text, g) = draw(&c, 120, 50);
    assert!(text.contains("Retain to preserve your chosen replacement"));
    let remove = g
        .compound_extension_review
        .unwrap()
        .shell
        .actions
        .into_iter()
        .find(|r| r.action.intent == CheckIntent::Remove)
        .unwrap();
    assert!(!remove.action.enabled);
    key(&mut c, KeyCode::Char('d'));
    key(&mut c, KeyCode::Char('r'));
    key(&mut c, KeyCode::Char('k'));
    key(&mut c, KeyCode::Enter);
    assert_eq!(c.preferences.compound_extensions, Some(vec![]));
    assert_eq!(
        c.preferences.compound_extensions_extend,
        [".tar.gz", ".user.js"]
    );
}

#[test]
fn additive_only_review_removes_only_explicitly_approved_default_entries() {
    for remove in [false, true] {
        let mut c = controller(
            "compound_extensions_extend = ['.tar.gz', '.tar.gz', '.user.js']",
            "0.1.0",
        );
        if remove {
            key(&mut c, KeyCode::Char('r'));
        }
        key(&mut c, KeyCode::Enter);
        assert!(c.preferences.compound_extensions.is_none());
        assert_eq!(
            c.preferences.compound_extensions_extend,
            if remove {
                vec![".user.js"]
            } else {
                vec![".tar.gz", ".tar.gz", ".user.js"]
            }
        );
    }
}

#[test]
fn dismissal_and_close_decline_tentative_changes_without_leaking_input() {
    for close in [false, true] {
        let mut c = controller(COMBINED, "0.1.0");
        let original = c.preferences.clone();
        key(&mut c, KeyCode::Char('d'));
        key(&mut c, KeyCode::Char('r'));
        if close {
            let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 50));
            let area = g.compound_extension_review.unwrap().shell.close.unwrap();
            event(
                &mut c,
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: area.x,
                    row: area.y,
                    modifiers: KeyModifiers::NONE,
                }),
                120,
                50,
            );
        } else {
            key(&mut c, KeyCode::Esc);
        }
        assert!(!c.quit);
        assert!(c.app.compound_extension_review.is_none());
        assert_eq!(
            c.preferences.compound_extensions,
            original.compound_extensions
        );
        assert_eq!(
            c.preferences.compound_extensions_extend,
            original.compound_extensions_extend
        );
        assert_eq!(
            c.preferences.compound_extensions_checked_version.as_deref(),
            Some("0.1.0")
        );
        c.check_compound_extensions("0.1.0").unwrap();
        assert!(c.app.compound_extension_review.is_none());
        c.check_compound_extensions("0.2.0").unwrap();
        assert!(c.app.compound_extension_review.is_some());
    }
}

#[test]
fn mouse_and_keyboard_use_the_same_controls_and_block_release_repeat_and_paste() {
    let mut c = controller(COMBINED, "0.1.0");
    for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
        event(
            &mut c,
            Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char('d'),
                KeyModifiers::NONE,
                kind,
            )),
            120,
            50,
        );
    }
    event(&mut c, Event::Paste("d\nr".into()), 120, 50);
    assert!(
        !c.app
            .compound_extension_review
            .as_ref()
            .unwrap()
            .use_defaults
    );
    click(&mut c, CheckIntent::Defaults);
    click(&mut c, CheckIntent::Remove);
    click(&mut c, CheckIntent::Continue);
    assert!(c.preferences.compound_extensions.is_none());
    assert_eq!(c.preferences.compound_extensions_extend, [".user.js"]);
}

fn click(c: &mut Controller, intent: CheckIntent) {
    let (_, g) = draw(c, 120, 50);
    let area = g
        .compound_extension_review
        .unwrap()
        .shell
        .actions
        .into_iter()
        .find(|r| r.action.intent == intent)
        .unwrap()
        .area;
    event(
        c,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }),
        120,
        50,
    );
}

#[test]
fn tab_space_and_keep_action_allow_independent_decline() {
    let mut c = controller(COMBINED, "0.1.0");
    key(&mut c, KeyCode::Char('d'));
    key(&mut c, KeyCode::Char('k'));
    let selected = c.app.compound_extension_review.as_ref().unwrap().selected;
    event(
        &mut c,
        Event::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        120,
        40,
    );
    assert_ne!(
        c.app.compound_extension_review.as_ref().unwrap().selected,
        selected
    );
    key(&mut c, KeyCode::Char(' '));
    key(&mut c, KeyCode::Tab);
    key(&mut c, KeyCode::Enter);
    assert!(c.app.compound_extension_review.is_none());
    assert!(c.preferences.compound_extensions.is_some());
    assert_eq!(
        c.preferences.compound_extensions_extend,
        [".tar.xz", ".tar.xz", ".user.js"]
    );
}

#[test]
fn interrupt_declines_and_exits_while_other_browser_actions_are_captured() {
    let mut c = controller(COMBINED, "0.1.0");
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 50));
    c.dispatch(terminal_file_manager::input::Action::Quit, &g);
    assert!(!c.quit);
    event(
        &mut c,
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        120,
        50,
    );
    assert!(c.quit);
    assert_eq!(
        c.preferences.compound_extensions,
        Some(vec![".tar.gz".into(), ".custom.js".into()])
    );
    assert_eq!(
        c.preferences.compound_extensions_checked_version.as_deref(),
        Some("0.1.0")
    );
}
