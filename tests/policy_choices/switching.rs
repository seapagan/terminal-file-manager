use super::{
    keyboard::{CASES, nondefault_policy},
    *,
};
use crossterm::event::KeyEventKind;
use terminal_file_manager::{input::HelpTopic, settings::Settings, view::ViewSettings};

const BOUNDS: Rect = Rect::new(0, 0, 240, 30);

fn prepared(pane: usize) -> Controller {
    let mut c = controller();
    c.app.dual = true;
    c.app.active = pane;
    nondefault_policy(&mut c, pane);
    c
}

fn assert_menu(c: &Controller, pane: usize, action: Action, selected: usize) {
    let menu = c.app.policy_menu.as_ref().expect("active popup");
    assert_eq!(
        (menu.pane, menu.action, menu.selected),
        (pane, action, selected)
    );
    assert_eq!(c.app.active, pane);
}

#[test]
fn all_popup_shortcuts_switch_or_close_and_discard_pending_choices_in_each_pane() {
    for pane in 0..2 {
        for &(source, _, source_action, source_current, _) in CASES {
            for &(target, _, target_action, target_current, _) in CASES {
                let mut c = prepared(pane);
                let before = c.app.panes.each_ref().map(ViewSettings::capture);
                key(&mut c, source, BOUNDS);
                key(&mut c, "Home", BOUNDS);
                key(&mut c, "Tab", BOUNDS);
                assert_menu(&c, pane, source_action, 0);
                key(&mut c, target, BOUNDS);
                if source == target {
                    assert!(c.app.policy_menu.is_none());
                } else {
                    assert_menu(&c, pane, target_action, target_current);
                }
                key(&mut c, source, BOUNDS);
                assert_menu(&c, pane, source_action, source_current);
                assert_eq!(c.app.panes.each_ref().map(ViewSettings::capture), before);
                assert!(!c.preferences_changed);
                assert!(c.app.operations.outbox.is_empty());
            }
        }
    }
}

#[test]
fn repeat_and_release_events_never_switch_or_toggle_the_popup() {
    for &(source, _, action, _, _) in CASES {
        for &(target, _, _, _, _) in CASES {
            let mut c = prepared(1);
            key(&mut c, source, BOUNDS);
            key(&mut c, "Home", BOUNDS);
            let (code, modifiers) = chord(target).unwrap();
            for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
                let g = ui::geometry(&c.app, BOUNDS);
                let mut event = KeyEvent::new(code, modifiers);
                event.kind = kind;
                assert!(
                    c.handle(Event::Key(event), &g, Default::default())
                        .is_empty()
                );
                assert_menu(&c, 1, action, 0);
                assert!(!c.preferences_changed);
            }
        }
    }
}

fn configured() -> Controller {
    let settings = Settings::parse("[bindings]\nS='sort'\nG='group'\nH='hidden'\nI='ignore'\n'Alt+s'='sort-menu'\n'Alt+g'='group-menu'\n'Alt+h'='hidden-menu'\n'Alt+i'='ignore-menu'").unwrap();
    let mut c = Controller::new(
        App::new("owned".into()),
        Bindings::new(&settings.bindings).unwrap(),
        settings,
    );
    c.app.dual = true;
    c.app.active = 1;
    nondefault_policy(&mut c, 1);
    c
}

#[test]
fn switching_and_toggle_use_effective_overrides_instead_of_default_letters() {
    for &(source, cycle, action, current, _) in CASES {
        for &(target, target_cycle, target_action, target_current, _) in CASES {
            let mut c = configured();
            let before = c.app.panes.each_ref().map(ViewSettings::capture);
            key(&mut c, &format!("Alt+{cycle}"), BOUNDS);
            key(&mut c, "Home", BOUNDS);
            for letter in ["S", "G", "H", "I"] {
                key(&mut c, letter, BOUNDS);
            }
            assert_menu(&c, 1, action, 0);
            key(&mut c, &format!("Alt+{target_cycle}"), BOUNDS);
            if source == target {
                assert!(c.app.policy_menu.is_none());
            } else {
                assert_menu(&c, 1, target_action, target_current);
            }
            key(&mut c, &format!("Alt+{cycle}"), BOUNDS);
            assert_menu(&c, 1, action, current);
            assert_eq!(c.app.panes.each_ref().map(ViewSettings::capture), before);
            assert!(!c.preferences_changed);
        }
    }
}

#[test]
fn contextual_help_owns_popup_shortcuts_and_escape_closes_one_layer_at_a_time() {
    for &(source, _, action, _, _) in CASES {
        let mut c = prepared(0);
        key(&mut c, source, BOUNDS);
        key(&mut c, "Home", BOUNDS);
        key(&mut c, "F1", BOUNDS);
        for &(target, _, _, _, _) in CASES {
            key(&mut c, target, BOUNDS);
        }
        assert_eq!(
            c.app.context_help.as_ref().unwrap().topic,
            HelpTopic::Policy(action)
        );
        assert_menu(&c, 0, action, 0);
        key(&mut c, "Esc", BOUNDS);
        assert!(c.app.context_help.is_none());
        assert_menu(&c, 0, action, 0);
        key(&mut c, "Esc", BOUNDS);
        assert!(c.app.policy_menu.is_none());
        assert!(!c.preferences_changed);
    }
}

#[test]
fn reserved_escape_help_and_emergency_exit_keep_priority_over_popup_bindings() {
    let settings =
        Settings::parse("[bindings]\nEsc='sort-menu'\n'Ctrl+c'='group-menu'\nF1='hidden-menu'")
            .unwrap();
    let mut c = Controller::new(
        App::new("owned".into()),
        Bindings::new(&settings.bindings).unwrap(),
        settings,
    );
    key(&mut c, "I", BOUNDS);
    key(&mut c, "F1", BOUNDS);
    assert_eq!(
        c.app.context_help.as_ref().unwrap().topic,
        HelpTopic::Policy(Action::Ignore)
    );
    key(&mut c, "Esc", BOUNDS);
    assert_menu(&c, 0, Action::Ignore, 0);
    key(&mut c, "Esc", BOUNDS);
    assert!(c.app.policy_menu.is_none());
    key(&mut c, "I", BOUNDS);
    key(&mut c, "Ctrl+c", BOUNDS);
    assert!(c.quit && c.app.policy_menu.is_none());
    assert!(!c.preferences_changed && c.app.operations.outbox.is_empty());
}

#[test]
fn clicking_another_policy_control_still_only_cancels_the_open_popup() {
    for &(_, _, source, _, _) in CASES {
        for &(_, _, target, _, _) in CASES {
            let mut c = controller();
            let g = ui::geometry(&c.app, BOUNDS);
            let control = |action| {
                g.policy_controls
                    .iter()
                    .find(|p| p.pane == 0 && p.action == action)
                    .unwrap()
                    .area
            };
            let from = control(source);
            let to = control(target);
            click(&mut c, from.x, from.y, BOUNDS);
            let g = ui::geometry(&c.app, BOUNDS);
            c.handle(
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::ScrollDown,
                    column: from.x,
                    row: from.y,
                    modifiers: KeyModifiers::NONE,
                }),
                &g,
                Default::default(),
            );
            assert_menu(&c, 0, source, 1);
            click(&mut c, to.x, to.y, BOUNDS);
            assert!(c.app.policy_menu.is_none());
            assert!(!c.preferences_changed);
            click(&mut c, to.x, to.y, BOUNDS);
            assert_menu(&c, 0, target, 0);
        }
    }
}
