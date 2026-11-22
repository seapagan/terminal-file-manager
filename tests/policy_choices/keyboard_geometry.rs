use super::*;
use terminal_file_manager::view::ViewSettings;

const CHOICES: &[(&str, Action, &str)] = &[
    ("S", Action::Sort, "Unsorted"),
    ("G", Action::Group, "Mixed"),
    ("H", Action::Hidden, "Show"),
    ("I", Action::Ignore, "Off"),
];

fn hidden_controls(pane: usize) -> Controller {
    let mut c = controller();
    c.app.dual = true;
    c.app.active = pane;
    c.app.split_percent = if pane == 0 { 20 } else { 80 };
    c.app.panes[pane].set_filter("x".repeat(256));
    c
}

fn draw(c: &Controller, bounds: Rect) -> (ratatui::buffer::Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    let mut g = ui::geometry(&c.app, bounds);
    terminal
        .draw(|frame| {
            g = ui::render(
                frame,
                &c.app,
                &Theme::new(colored_text::ColorLevel::NoColor),
            )
        })
        .unwrap();
    (terminal.backend().buffer().clone(), g)
}

#[test]
fn hidden_policy_controls_have_bounded_keyboard_popups_in_each_active_pane() {
    for pane in 0..2 {
        for width in [1, 4, 12, 24, 40, 80, 120] {
            for height in [1, 5, 9, 24] {
                for &(shortcut, action, _) in CHOICES {
                    let bounds = Rect::new(0, 0, width, height);
                    let mut c = hidden_controls(pane);
                    let g = ui::geometry(&c.app, bounds);
                    assert!(
                        !g.policy_controls
                            .iter()
                            .any(|control| control.pane == pane && control.action == action)
                    );
                    key(&mut c, shortcut, bounds);
                    let (_, g) = draw(&c, bounds);
                    let owner = g.panes[pane].unwrap();
                    let popup = g.policy_menu.unwrap();
                    assert_eq!(popup.shell.outer.intersection(owner), popup.shell.outer);
                    assert_eq!(popup.shell.outer.intersection(bounds), popup.shell.outer);
                    assert_eq!(popup.shell.outer.x, owner.x);
                    key(&mut c, "Esc", bounds);
                    assert!(c.app.policy_menu.is_none());
                    assert!(!c.preferences_changed);
                }
            }
        }
    }
}

#[test]
fn hidden_control_popups_scroll_render_help_and_apply_the_clicked_final_option() {
    let bounds = Rect::new(0, 0, 80, 9);
    for pane in 0..2 {
        for &(shortcut, _, last) in CHOICES {
            let mut c = hidden_controls(pane);
            let other = ViewSettings::capture(&c.app.panes[1 - pane]);
            key(&mut c, shortcut, bounds);
            key(&mut c, "End", bounds);
            let (buffer, g) = draw(&c, bounds);
            let popup = g.policy_menu.unwrap();
            let body = popup.shell.body;
            let row: String = (body.x..body.right())
                .map(|x| buffer[(x, body.bottom() - 1)].symbol())
                .collect();
            assert!(row.contains(last), "{row}");
            let selected = c.app.policy_menu.as_ref().unwrap().selected;
            key(&mut c, "F1", bounds);
            assert!(c.app.context_help.is_some());
            key(&mut c, "Esc", bounds);
            assert_eq!(c.app.policy_menu.as_ref().unwrap().selected, selected);
            let restored = ui::geometry(&c.app, bounds).policy_menu.unwrap();
            assert_eq!(restored.shell.body, body);
            assert_eq!(restored.start, popup.start);
            let help = restored.shell.help.unwrap();
            click(&mut c, help.x, help.y, bounds);
            assert!(c.app.context_help.is_some());
            key(&mut c, "Esc", bounds);
            assert_eq!(c.app.policy_menu.as_ref().unwrap().selected, selected);
            click(&mut c, body.x, body.bottom() - 1, bounds);
            assert!(c.app.policy_menu.is_none() && c.preferences_changed);
            assert_eq!(ViewSettings::capture(&c.app.panes[1 - pane]), other);
            key(&mut c, shortcut, bounds);
            assert_eq!(c.app.policy_menu.as_ref().unwrap().selected, selected);
            assert!(c.app.operations.outbox.is_empty());
        }
    }
}

#[test]
fn enter_cannot_apply_a_choice_when_the_fallback_has_no_content_cells() {
    for width in [1, 2, 4] {
        let bounds = Rect::new(0, 0, width, 9);
        for &(shortcut, _, _) in CHOICES {
            let mut c = hidden_controls(0);
            let before = c.app.panes.each_ref().map(ViewSettings::capture);
            key(&mut c, shortcut, bounds);
            let popup = ui::geometry(&c.app, bounds).policy_menu.unwrap();
            assert_eq!(popup.shell.body.width, 0);
            key(&mut c, "End", bounds);
            key(&mut c, "Enter", bounds);
            assert!(c.app.policy_menu.is_none());
            assert!(!c.preferences_changed);
            assert_eq!(c.app.panes.each_ref().map(ViewSettings::capture), before);
        }
    }
}
