use super::*;
use crossterm::event::{KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use terminal_file_manager::{settings::compound_check::Stage, ui::CheckIntent};

const COMBINED: &str = "sidebar = true\nmouse = false\ncompound_extensions = ['.tar.gz', '.custom.js']\ncompound_extensions_extend = ['.tar.gz', '.tar.xz', '.tar.xz', '.user.js']";

#[test]
fn sequential_choices_are_independent_and_cleanup_follows_the_selected_base() {
    for defaults in [false, true] {
        for remove in [false, true] {
            let mut c = controller(COMBINED, "0.1.0");
            let before = c.preferences.effective_compound_extensions();
            key(&mut c, KeyCode::Char(if defaults { 'd' } else { 'k' }));
            assert_eq!(
                c.app.compound_extension_review.as_ref().unwrap().stage,
                Stage::Additions
            );
            assert!(c.preferences.compound_extensions_checked_version.is_none());
            key(&mut c, KeyCode::Char(if remove { 'r' } else { 'k' }));
            assert!(c.app.compound_extension_review.is_none());
            assert_eq!(
                c.preferences.compound_extensions,
                if defaults {
                    None
                } else {
                    Some(vec![".tar.gz".into(), ".custom.js".into()])
                }
            );
            let additions = match (defaults, remove) {
                (_, false) => vec![".tar.gz", ".tar.xz", ".tar.xz", ".user.js"],
                (false, true) => vec![".tar.xz", ".tar.xz", ".user.js"],
                (true, true) => vec![".user.js"],
            };
            assert_eq!(c.preferences.compound_extensions_extend, additions);
            if !defaults {
                assert_eq!(c.preferences.effective_compound_extensions(), before);
            }
            assert!(c.preferences.sidebar && !c.preferences.mouse);
            let saved = Settings::parse(&toml::to_string(&c.preferences).unwrap()).unwrap();
            assert_eq!(saved.compound_extensions, c.preferences.compound_extensions);
            assert_eq!(saved.compound_extensions_extend, additions);
        }
    }
}

#[test]
fn excluded_defaults_skip_cleanup_when_the_replacement_is_retained() {
    for defaults in [false, true] {
        let mut c = controller(
            "compound_extensions = []\ncompound_extensions_extend = ['.tar.gz', '.user.js']",
            "0.1.0",
        );
        key(&mut c, KeyCode::Char('r'));
        assert_eq!(
            c.app.compound_extension_review.as_ref().unwrap().stage,
            Stage::Replacement
        );
        key(&mut c, KeyCode::Char(if defaults { 'd' } else { 'k' }));
        if defaults {
            assert_eq!(
                c.app
                    .compound_extension_review
                    .as_ref()
                    .unwrap()
                    .removable(),
                [".tar.gz"]
            );
            key(&mut c, KeyCode::Char('r'));
        } else {
            assert!(c.app.compound_extension_review.is_none());
        }
        assert_eq!(
            c.preferences.compound_extensions_extend,
            if defaults {
                vec![".user.js"]
            } else {
                vec![".tar.gz", ".user.js"]
            }
        );
        assert_eq!(
            c.preferences.compound_extensions,
            if defaults { None } else { Some(vec![]) }
        );
    }
}

#[test]
fn cleanup_covers_redundancies_in_a_custom_effective_base() {
    let mut c = controller(
        "compound_extensions = ['.custom.js']\ncompound_extensions_extend = ['.custom.js', '.tar.gz']",
        "0.1.0",
    );
    key(&mut c, KeyCode::Enter);
    assert_eq!(
        c.app
            .compound_extension_review
            .as_ref()
            .unwrap()
            .removable(),
        [".custom.js"]
    );
    key(&mut c, KeyCode::Char('r'));
    assert_eq!(c.preferences.compound_extensions_extend, [".tar.gz"]);
}

#[test]
fn additive_only_actions_commit_immediately_without_an_extra_continue() {
    for remove in [false, true] {
        let mut c = controller(
            "compound_extensions_extend = ['.tar.gz', '.tar.gz', '.user.js']",
            "0.1.0",
        );
        assert_eq!(
            c.app.compound_extension_review.as_ref().unwrap().stage,
            Stage::Additions
        );
        key(&mut c, KeyCode::Char(if remove { 'r' } else { 'k' }));
        assert!(c.app.compound_extension_review.is_none());
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
fn safe_default_advances_from_replacement_to_a_separate_additions_dialog() {
    let mut c = controller(COMBINED, "0.1.0");
    key(&mut c, KeyCode::Enter);
    assert!(c.app.compound_extension_review.is_some());
    let (text, g) = draw(&c, 120, 40);
    assert!(text.contains("Review custom extensions"), "{text}");
    assert_eq!(g.compound_extension_review.unwrap().shell.actions.len(), 2);
    assert!(c.preferences.compound_extensions_checked_version.is_none());
    key(&mut c, KeyCode::Enter);
    assert!(c.app.compound_extension_review.is_none());
    assert_eq!(
        c.preferences.compound_extensions_extend,
        [".tar.gz", ".tar.xz", ".tar.xz", ".user.js"]
    );
    c.check_compound_extensions("0.1.0").unwrap();
    assert!(c.app.compound_extension_review.is_none());
    c.check_compound_extensions("0.2.0").unwrap();
    assert_eq!(
        c.app.compound_extension_review.unwrap().stage,
        Stage::Replacement
    );
}

#[test]
fn escape_and_close_keep_each_dialog_without_reverting_an_approved_replacement() {
    for close in [false, true] {
        for defaults in [false, true] {
            let mut c = controller(COMBINED, "0.1.0");
            if defaults {
                key(&mut c, KeyCode::Char('d'));
            } else {
                dismiss(&mut c, close);
            }
            assert_eq!(
                c.app.compound_extension_review.as_ref().unwrap().stage,
                Stage::Additions
            );
            dismiss(&mut c, close);
            assert!(!c.quit);
            assert!(c.app.compound_extension_review.is_none());
            assert_eq!(
                c.preferences.compound_extensions,
                if defaults {
                    None
                } else {
                    Some(vec![".tar.gz".into(), ".custom.js".into()])
                }
            );
            assert_eq!(
                c.preferences.compound_extensions_extend,
                [".tar.gz", ".tar.xz", ".tar.xz", ".user.js"]
            );
            assert_eq!(
                c.preferences.compound_extensions_checked_version.as_deref(),
                Some("0.1.0")
            );
        }
    }
}

fn dismiss(c: &mut Controller, close: bool) {
    if close {
        let (_, g) = draw(c, 120, 40);
        let area = g.compound_extension_review.unwrap().shell.close.unwrap();
        mouse(c, area.x, area.y);
    } else {
        key(c, KeyCode::Esc);
    }
}

fn mouse(c: &mut Controller, column: u16, row: u16) {
    event(
        c,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }),
        120,
        40,
    );
}

fn click(c: &mut Controller, intent: CheckIntent) {
    let (_, g) = draw(c, 120, 40);
    let area = g
        .compound_extension_review
        .unwrap()
        .shell
        .actions
        .into_iter()
        .find(|r| r.action.intent == intent)
        .unwrap()
        .area;
    mouse(c, area.x, area.y);
}

#[test]
fn mouse_choices_use_measured_controls_and_release_repeat_and_paste_cannot_advance() {
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
            40,
        );
    }
    event(&mut c, Event::Paste("d\nr".into()), 120, 40);
    assert_eq!(
        c.app.compound_extension_review.as_ref().unwrap().stage,
        Stage::Replacement
    );
    click(&mut c, CheckIntent::Defaults);
    click(&mut c, CheckIntent::Remove);
    assert!(c.app.compound_extension_review.is_none());
    assert!(c.preferences.compound_extensions.is_none());
    assert_eq!(c.preferences.compound_extensions_extend, [".user.js"]);
}

#[test]
fn tab_backtab_and_space_select_one_action_then_reset_to_the_safe_default() {
    let mut c = controller(COMBINED, "0.1.0");
    key(&mut c, KeyCode::Tab);
    event(
        &mut c,
        Event::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        120,
        40,
    );
    assert_eq!(
        c.app.compound_extension_review.as_ref().unwrap().selected,
        0
    );
    key(&mut c, KeyCode::Right);
    key(&mut c, KeyCode::Char(' '));
    assert_eq!(
        c.app.compound_extension_review.as_ref().unwrap().stage,
        Stage::Additions
    );
    assert_eq!(
        c.app.compound_extension_review.as_ref().unwrap().selected,
        0
    );
    key(&mut c, KeyCode::Enter);
    assert!(c.preferences.compound_extensions.is_none());
    assert_eq!(
        c.preferences.compound_extensions_extend,
        [".tar.gz", ".tar.xz", ".tar.xz", ".user.js"]
    );
}

#[test]
fn controls_from_the_preceding_dialog_cannot_activate_the_next_dialog() {
    let mut c = controller(COMBINED, "0.1.0");
    let (_, g) = draw(&c, 120, 40);
    key(&mut c, KeyCode::Char('d'));
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &g,
        std::time::Duration::ZERO,
    );
    assert_eq!(
        c.app.compound_extension_review.as_ref().unwrap().stage,
        Stage::Additions
    );
}

#[test]
fn interrupt_declines_the_current_dialog_and_preserves_prior_approved_choices() {
    for defaults in [false, true] {
        let mut c = controller(COMBINED, "0.1.0");
        let (_, g) = draw(&c, 120, 40);
        c.dispatch(terminal_file_manager::input::Action::Quit, &g);
        assert!(!c.quit);
        if defaults {
            key(&mut c, KeyCode::Char('d'));
        }
        event(
            &mut c,
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            120,
            40,
        );
        assert!(c.quit);
        assert_eq!(
            c.preferences.compound_extensions,
            if defaults {
                None
            } else {
                Some(vec![".tar.gz".into(), ".custom.js".into()])
            }
        );
        assert_eq!(
            c.preferences.compound_extensions_extend,
            [".tar.gz", ".tar.xz", ".tar.xz", ".user.js"]
        );
        assert_eq!(
            c.preferences.compound_extensions_checked_version.as_deref(),
            Some("0.1.0")
        );
    }
}
