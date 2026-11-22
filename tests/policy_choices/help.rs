use super::*;
use terminal_file_manager::input::HelpTopic;

const ACTIONS: [Action; 4] = [Action::Sort, Action::Group, Action::Hidden, Action::Ignore];

#[test]
fn f1_and_shared_border_help_preserve_the_highlighted_choice_and_policy() {
    let bounds = Rect::new(0, 0, 120, 30);
    for action in ACTIONS {
        let mut c = controller();
        compact::open(&mut c, action, bounds);
        key(&mut c, "End", bounds);
        let selected = c.app.policy_menu.as_ref().unwrap().selected;
        let before = ui::geometry(&c.app, bounds)
            .policy_menu
            .unwrap()
            .shell
            .outer;
        key(&mut c, "F1", bounds);
        assert_eq!(
            c.app.context_help.as_ref().unwrap().topic,
            HelpTopic::Policy(action)
        );
        key(&mut c, "Home", bounds);
        key(&mut c, "Esc", bounds);
        assert!(c.app.context_help.is_none());
        let g = ui::geometry(&c.app, bounds);
        assert_eq!(g.policy_menu.as_ref().unwrap().shell.outer, before);
        let help = g.context_control.unwrap();
        assert_eq!(help, g.policy_menu.as_ref().unwrap().shell.help.unwrap());
        click(&mut c, help.x, help.y, bounds);
        assert_eq!(
            c.app.context_help.as_ref().unwrap().topic,
            HelpTopic::Policy(action)
        );
        click(&mut c, 0, 0, bounds);
        assert_eq!(c.app.policy_menu.as_ref().unwrap().selected, selected);
        key(&mut c, "Esc", bounds);
        assert!(c.app.context_help.is_none());
        assert_eq!(c.app.policy_menu.as_ref().unwrap().selected, selected);
        assert!(!c.preferences_changed);
        assert!(!c.app.help && c.app.operations.outbox.is_empty());
    }
}

#[test]
fn shared_help_button_fits_the_dynamic_border_and_help_title_is_rendered() {
    let bounds = Rect::new(0, 0, 120, 30);
    for action in ACTIONS {
        let mut c = controller();
        compact::open(&mut c, action, bounds);
        let g = drawn(&c, bounds).0;
        let shell = &g.policy_menu.as_ref().unwrap().shell;
        let help = shell.help.unwrap();
        assert_eq!(help.right(), shell.outer.right() - 1);
        assert_eq!(help.y, shell.outer.y);
        assert_eq!(help.width, 3);
        let (_, rows) = drawn(&c, bounds);
        assert!(rows[usize::from(help.y)].contains("[?]"));
        assert!(!rows.iter().any(|row| row.contains("Choose")));
        click(&mut c, help.x + 1, help.y, bounds);
        let (g, rows) = drawn(&c, bounds);
        assert!(g.context_help.is_some());
        assert!(
            rows.iter()
                .any(|row| row.contains(HelpTopic::Policy(action).title()))
        );
    }
}

#[test]
fn narrow_scrolled_policy_menu_returns_from_help_without_committing() {
    let mut c = controller();
    let large = Rect::new(0, 0, 120, 30);
    compact::open(&mut c, Action::Sort, large);
    key(&mut c, "End", large);
    let bounds = Rect::new(0, 0, 32, 9);
    let before = ui::geometry(&c.app, bounds).policy_menu.unwrap();
    assert!(before.start > 0);
    key(&mut c, "F1", bounds);
    let g = ui::geometry(&c.app, bounds);
    let help = g.context_help.unwrap();
    assert_eq!(help.shell.outer.intersection(bounds), help.shell.outer);
    key(&mut c, "End", bounds);
    assert!(c.app.context_help.as_ref().unwrap().scroll > 0);
    key(&mut c, "Esc", bounds);
    let after = ui::geometry(&c.app, bounds).policy_menu.unwrap();
    assert_eq!(before.shell.outer, after.shell.outer);
    assert_eq!(before.start, after.start);
    assert_eq!(c.app.policy_menu.as_ref().unwrap().selected, 8);
    assert!(!c.preferences_changed);
}

#[test]
fn contextual_help_explains_every_option_and_the_key_policy_distinctions() {
    let c = controller();
    for (action, options, distinction) in [
        (
            Action::Sort,
            "Natural Name Extension Size Modified Created Kind Version Unsorted",
            "filesystem's directory-listing order",
        ),
        (Action::Group, "First Last Mixed", "enterable directories"),
        (Action::Hidden, "Hide Show", "hidden entries"),
        (Action::Ignore, "Dim Hide Off", "normal name styling"),
    ] {
        let text = HelpTopic::Policy(action)
            .text(&c.app)
            .into_iter()
            .flatten()
            .map(|fragment| fragment.text().to_owned())
            .collect::<String>();
        for option in options.split_whitespace() {
            assert!(text.contains(option), "{action:?}: {option}");
        }
        assert!(text.contains(distinction));
        assert!(text.contains("Esc") && text.contains("highlighted choice"));
        if action == Action::Sort {
            assert!(text.contains("file2 comes before file10"));
        }
        if action == Action::Ignore {
            assert!(text.contains("subdued names") && text.contains("excludes them"));
        }
    }
}

fn drawn(c: &Controller, bounds: Rect) -> (ui::Geometry, Vec<String>) {
    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    let mut geometry = ui::geometry(&c.app, bounds);
    terminal
        .draw(|frame| {
            geometry = ui::render(
                frame,
                &c.app,
                &Theme::new(colored_text::ColorLevel::NoColor),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rows = (0..bounds.height)
        .map(|y| (0..bounds.width).map(|x| buffer[(x, y)].symbol()).collect())
        .collect();
    (geometry, rows)
}

#[test]
fn f1_stays_available_when_the_terminal_cannot_show_a_help_button() {
    let mut c = controller();
    compact::open(&mut c, Action::Sort, Rect::new(0, 0, 120, 30));
    let bounds = Rect::new(0, 0, 4, 8);
    assert!(ui::geometry(&c.app, bounds).context_control.is_none());
    key(&mut c, "F1", bounds);
    assert_eq!(
        c.app.context_help.as_ref().unwrap().topic,
        HelpTopic::Policy(Action::Sort)
    );
    key(&mut c, "Esc", bounds);
    assert!(c.app.context_help.is_none());
    assert!(c.app.policy_menu.is_some());
    assert!(!c.preferences_changed);
}
