use super::*;
use terminal_file_manager::{app::IgnoreMode, view::ViewSettings};
use terminal_file_manager::{
    input::{HelpFragment, HelpSection, HelpTopic},
    settings::Settings,
};

pub(super) const CASES: &[(&str, &str, Action, usize, usize)] = &[
    ("S", "s", Action::Sort, 7, 8),
    ("G", "g", Action::Group, 1, 2),
    ("H", "h", Action::Hidden, 1, 0),
    ("I", "i", Action::Ignore, 1, 2),
];

pub(super) fn nondefault_policy(c: &mut Controller, pane: usize) {
    let pane = &mut c.app.panes[pane];
    pane.sort.sort = Sort::Version;
    pane.sort.grouping = Grouping::Last;
    pane.show_hidden = true;
    pane.ignore = IgnoreMode::Hide;
    pane.resort();
}

#[test]
fn popup_actions_use_configured_bindings_and_effective_help_labels() {
    let bounds = Rect::new(0, 0, 120, 30);
    for (upper, direct, menu, name) in [
        ("S", "sort", Action::SortMenu, "sort-menu"),
        ("G", "group", Action::GroupMenu, "group-menu"),
        ("H", "hidden", Action::HiddenMenu, "hidden-menu"),
        ("I", "ignore", Action::IgnoreMenu, "ignore-menu"),
    ] {
        let settings =
            Settings::parse(&format!("[bindings]\n'{upper}'='{direct}'\nF11='{name}'")).unwrap();
        let bindings = Bindings::new(&settings.bindings).unwrap();
        let mut c = Controller::new(App::new("owned".into()), bindings, settings);
        key(&mut c, upper, bounds);
        assert!(
            c.app.policy_menu.is_none(),
            "uppercase override must change directly"
        );
        key(&mut c, "F11", bounds);
        let popup = c.app.policy_menu.as_ref().unwrap();
        assert_eq!(popup.selected, 1);
        let policy = popup.action;
        let entry = c.app.help_entry(menu).unwrap();
        assert_eq!(entry.keys, "F11");
        assert_eq!(entry.action.help_section(), HelpSection::Views);
        assert!(entry.description.contains("popup"));
        key(&mut c, "F1", bounds);
        assert_eq!(
            c.app.context_help.as_ref().unwrap().topic,
            HelpTopic::Policy(policy)
        );
        let text: String = HelpTopic::Policy(policy)
            .text(&c.app)
            .into_iter()
            .flatten()
            .map(|fragment| match fragment {
                HelpFragment::Text(text) | HelpFragment::Key(text) => text,
            })
            .collect();
        assert!(text.contains("Open from the file list: F11."));
        assert!(text.contains("Change directly from the file list:"));
    }
}

#[test]
fn uppercase_legacy_and_shift_encodings_share_the_configurable_popup_actions() {
    let bindings = Bindings::new(&Default::default()).unwrap();
    for (letter, action) in [
        ('s', Action::SortMenu),
        ('g', Action::GroupMenu),
        ('h', Action::HiddenMenu),
        ('i', Action::IgnoreMenu),
    ] {
        for (character, modifiers) in [
            (letter, KeyModifiers::SHIFT),
            (letter.to_ascii_uppercase(), KeyModifiers::NONE),
            (letter.to_ascii_uppercase(), KeyModifiers::SHIFT),
        ] {
            assert_eq!(
                bindings.action(KeyEvent::new(
                    crossterm::event::KeyCode::Char(character),
                    modifiers
                )),
                Some(action)
            );
        }
    }
}

#[test]
fn keyboard_and_mouse_open_identical_popups_and_apply_to_only_the_owner() {
    let bounds = Rect::new(0, 0, 240, 30);
    for pane in 0..2 {
        for &(shortcut, _, action, _, _) in CASES {
            let mut keyboard = controller();
            keyboard.app.dual = true;
            keyboard.app.active = pane;
            let untouched = ViewSettings::capture(&keyboard.app.panes[1 - pane]);
            let mut mouse = controller();
            mouse.app.dual = true;
            let g = ui::geometry(&mouse.app, bounds);
            let control = g
                .policy_controls
                .iter()
                .find(|control| control.pane == pane && control.action == action)
                .unwrap();
            click(&mut mouse, control.area.x, control.area.y, bounds);
            key(&mut keyboard, shortcut, bounds);
            let mouse_layout = ui::geometry(&mouse.app, bounds).policy_menu.unwrap();
            let keyboard_layout = ui::geometry(&keyboard.app, bounds).policy_menu.unwrap();
            assert_eq!(mouse_layout.shell.outer, keyboard_layout.shell.outer);
            assert_eq!(mouse_layout.shell.body, keyboard_layout.shell.body);
            key(&mut keyboard, "Home", bounds);
            key(&mut keyboard, "Down", bounds);
            key(&mut keyboard, "Up", bounds);
            key(&mut keyboard, "End", bounds);
            key(&mut keyboard, "Enter", bounds);
            click(
                &mut mouse,
                mouse_layout.shell.body.x,
                mouse_layout.shell.body.bottom() - 1,
                bounds,
            );
            assert_eq!(
                keyboard.app.panes.each_ref().map(ViewSettings::capture),
                mouse.app.panes.each_ref().map(ViewSettings::capture)
            );
            assert_eq!(
                ViewSettings::capture(&keyboard.app.panes[1 - pane]),
                untouched
            );
            assert!(keyboard.app.policy_menu.is_none() && mouse.app.policy_menu.is_none());
            assert_eq!(
                toml::to_string(&keyboard.preferences).unwrap(),
                toml::to_string(&mouse.preferences).unwrap()
            );
        }
    }
}

#[test]
fn uppercase_opens_each_active_policy_and_lowercase_keeps_direct_changes() {
    let bounds = Rect::new(0, 0, 240, 30);
    for pane in 0..2 {
        for &(open, cycle, action, selected, next) in CASES {
            let mut c = controller();
            c.app.dual = true;
            c.app.active = pane;
            nondefault_policy(&mut c, pane);
            let before = c.app.panes.each_ref().map(ViewSettings::capture);
            key(&mut c, open, bounds);
            let menu = c.app.policy_menu.as_ref().expect("keyboard policy popup");
            assert_eq!(
                (menu.pane, menu.action, menu.selected),
                (pane, action, selected)
            );
            assert_eq!(c.app.panes.each_ref().map(ViewSettings::capture), before);
            key(&mut c, "Esc", bounds);
            assert!(c.app.policy_menu.is_none());
            assert!(!c.preferences_changed);
            key(&mut c, cycle, bounds);
            assert!(c.app.policy_menu.is_none());
            key(&mut c, open, bounds);
            assert_eq!(c.app.policy_menu.as_ref().unwrap().selected, next);
            assert_eq!(
                ViewSettings::capture(&c.app.panes[1 - pane]),
                before[1 - pane]
            );
        }
    }
}
