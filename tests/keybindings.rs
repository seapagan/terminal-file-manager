//! Configured chords and terminal encodings share printable-key normalization.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers as Mods};
use std::collections::BTreeMap;
use terminal_file_manager::input::{Action, Bindings, chord};

fn bindings(pairs: &[(&str, Action)]) -> Bindings {
    let overrides = pairs
        .iter()
        .map(|(key, action)| ((*key).into(), *action))
        .collect();
    Bindings::new(&overrides).unwrap()
}

fn letter(text: &str, modifiers: Mods) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(text.chars().next().unwrap()), modifiers)
}

#[test]
fn configured_shifted_letter_matches_modern_and_legacy_events() {
    let keys = bindings(&[("Shift+x", Action::Quit), ("x", Action::Help)]);
    for (code, modifiers) in [("X", Mods::SHIFT), ("X", Mods::NONE), ("x", Mods::SHIFT)] {
        assert_eq!(keys.action(letter(code, modifiers)), Some(Action::Quit));
    }
    assert_eq!(keys.action(letter("x", Mods::NONE)), Some(Action::Help));
}

#[test]
fn ordinary_lowercase_and_uppercase_bindings_remain_distinct() {
    let keys = bindings(&[("x", Action::Mark), ("X", Action::Refresh)]);
    assert_eq!(keys.action(letter("x", Mods::NONE)), Some(Action::Mark));
    for modifiers in [Mods::NONE, Mods::SHIFT] {
        assert_eq!(keys.action(letter("X", modifiers)), Some(Action::Refresh));
    }
    assert_eq!(keys.action(letter("x", Mods::SHIFT)), Some(Action::Refresh));
}

#[test]
fn equivalent_shifted_spellings_conflict_and_distinct_modifiers_do_not() {
    for alias in ["X", "Shift+X", "shift+x"] {
        assert_eq!(chord("Shift+x"), chord(alias));
        let overrides = BTreeMap::from([
            ("Shift+x".into(), Action::Quit),
            (alias.into(), Action::Help),
        ]);
        assert!(
            Bindings::new(&overrides)
                .err()
                .unwrap()
                .contains("conflicting keybinding")
        );
    }
    let keys = bindings(&[
        ("Ctrl+Alt+Shift+x", Action::Quit),
        ("Ctrl+Alt+x", Action::Help),
    ]);
    for modifiers in [
        Mods::CONTROL | Mods::ALT,
        Mods::CONTROL | Mods::ALT | Mods::SHIFT,
    ] {
        assert_eq!(keys.action(letter("X", modifiers)), Some(Action::Quit));
    }
    assert_eq!(
        keys.action(letter("x", Mods::CONTROL | Mods::ALT)),
        Some(Action::Help)
    );
    assert_eq!(keys.action(letter("X", Mods::ALT)), None);
}

#[test]
fn ambiguous_nonletter_shift_is_rejected_and_printed_symbols_match_both_protocols() {
    for text in ["Shift+?", "Shift+1", "Shift+Space", "Alt+Shift+§"] {
        let error = Bindings::new(&BTreeMap::from([(text.into(), Action::Quit)]))
            .err()
            .expect("nonportable shifted symbol");
        assert!(
            error.contains("use the produced character without Shift"),
            "{error}"
        );
    }
    let keys = bindings(&[("?", Action::Quit), ("§", Action::Mark)]);
    for modifiers in [Mods::NONE, Mods::SHIFT] {
        assert_eq!(keys.action(letter("?", modifiers)), Some(Action::Quit));
        assert_eq!(keys.action(letter("§", modifiers)), Some(Action::Mark));
    }
    assert_eq!(
        keys.action(KeyEvent::new(KeyCode::Up, Mods::SHIFT)),
        Some(Action::ExtendUp)
    );
    for (text, action) in [
        ("Q", Action::SaveQuit),
        ("V", Action::ViewOptions),
        ("Z", Action::OppositeZoxide),
    ] {
        for modifiers in [Mods::NONE, Mods::SHIFT] {
            assert_eq!(keys.action(letter(text, modifiers)), Some(action));
        }
    }
}
