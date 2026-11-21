//! Mouse menus use policy geometry while keyboard policy shortcuts keep cycling.
#[path = "policy_choices/compact.rs"]
mod compact;
#[path = "policy_choices/dimensions.rs"]
mod dimensions;
#[path = "policy_choices/direct.rs"]
mod direct;
#[path = "policy_choices/priority.rs"]
mod priority;
use crossterm::event::{Event, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Action, Bindings, Controller, chord},
    listing::{Grouping, Sort},
    ui,
};
fn controller() -> Controller {
    Controller::new(
        App::new("owned".into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}
fn key(c: &mut Controller, text: &str, bounds: Rect) {
    let (code, modifiers) = chord(text).unwrap();
    let g = ui::geometry(&c.app, bounds);
    assert!(
        c.handle(
            Event::Key(KeyEvent::new(code, modifiers)),
            &g,
            Default::default()
        )
        .is_empty()
    );
}
fn click(c: &mut Controller, x: u16, y: u16, bounds: Rect) {
    let g = ui::geometry(&c.app, bounds);
    assert!(
        c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE
            }),
            &g,
            Default::default()
        )
        .is_empty()
    );
}
#[test]
fn policy_click_opens_upwards_and_keyboard_cycle_does_not() {
    let mut c = controller();
    let bounds = Rect::new(0, 0, 120, 30);
    key(&mut c, "s", bounds);
    assert_eq!(c.app.panes[0].sort.sort, Sort::Name);
    assert!(c.app.policy_menu.is_none());
    let g = ui::geometry(&c.app, bounds);
    let control = g
        .policy_controls
        .iter()
        .find(|control| control.action == Action::Sort)
        .unwrap();
    click(&mut c, control.area.x, control.area.y, bounds);
    let menu = ui::geometry(&c.app, bounds).policy_menu.unwrap();
    assert!(menu.shell.outer.bottom() <= control.area.y);
    assert_eq!(c.app.policy_menu.as_ref().unwrap().selected, 1);
    key(&mut c, "End", bounds);
    key(&mut c, "Enter", bounds);
    assert_eq!(c.app.panes[0].sort.sort, Sort::Unsorted);
    assert_eq!(c.preferences.sort, Sort::Unsorted);
    assert!(c.app.policy_menu.is_none());
    assert!(c.app.operations.outbox.is_empty());
}
#[test]
fn every_control_mouse_choice_matches_drawn_rows_and_cancel_is_inert() {
    let bounds = Rect::new(0, 0, 120, 30);
    for action in [Action::Sort, Action::Group, Action::Hidden, Action::Ignore] {
        let mut c = controller();
        c.app.panes[0].profile = Some("Detailed".into());
        let g = ui::geometry(&c.app, bounds);
        let control = g
            .policy_controls
            .iter()
            .find(|control| control.action == action)
            .unwrap();
        click(&mut c, control.area.x, control.area.y, bounds);
        key(&mut c, "Down", bounds);
        let mut t = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let mut drawn = ui::geometry(&c.app, bounds);
        t.draw(|f| drawn = ui::render(f, &c.app, &Theme::new(colored_text::ColorLevel::NoColor)))
            .unwrap();
        let layout = drawn.policy_menu.unwrap();
        assert!(layout.shell.outer.intersection(bounds) == layout.shell.outer);
        click(&mut c, layout.shell.body.x, layout.shell.body.y + 1, bounds);
        assert!(c.preferences_changed && c.app.panes[0].profile_modified);
        assert!(c.app.policy_menu.is_none());
        match action {
            Action::Sort => assert_eq!(c.app.panes[0].sort.sort, Sort::Name),
            Action::Group => assert_eq!(c.app.panes[0].sort.grouping, Grouping::Last),
            Action::Hidden => assert!(c.app.panes[0].show_hidden),
            Action::Ignore => assert_eq!(
                c.app.panes[0].ignore,
                terminal_file_manager::app::IgnoreMode::Hide
            ),
            _ => unreachable!(),
        }
        click(&mut c, control.area.x, control.area.y, bounds);
        key(&mut c, "Esc", bounds);
        assert!(c.app.policy_menu.is_none());
    }
}
#[test]
fn short_choice_viewport_scrolls_to_last_value_and_remains_bounded_after_resize() {
    let mut c = controller();
    let bounds = Rect::new(3, 5, 120, 12);
    let g = ui::geometry(&c.app, bounds);
    let control = g
        .policy_controls
        .iter()
        .find(|control| control.action == Action::Sort)
        .unwrap();
    click(&mut c, control.area.x, control.area.y, bounds);
    key(&mut c, "End", bounds);
    let layout = ui::geometry(&c.app, bounds).policy_menu.unwrap();
    assert!(layout.start > 0);
    assert_eq!(layout.shell.outer.intersection(bounds), layout.shell.outer);
    key(&mut c, "Enter", bounds);
    assert_eq!(c.app.panes[0].sort.sort, Sort::Unsorted);
}
