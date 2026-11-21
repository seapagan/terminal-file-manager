use crate::{
    app::App,
    input::{Action, Bindings, Controller, PolicyMenu},
    ui,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

#[test]
fn selecting_each_policy_rebuilds_once_and_reselecting_is_inert() {
    for (action, last) in [
        (Action::Sort, 8),
        (Action::Group, 2),
        (Action::Hidden, 1),
        (Action::Ignore, 2),
    ] {
        let mut c = Controller::new(
            App::new("owned".into()),
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        );
        for changes in [true, false] {
            c.preferences_changed = false;
            c.app.policy_menu = Some(PolicyMenu {
                pane: 0,
                action,
                selected: last,
            });
            let revision = c.app.panes[0].selection_revision;
            let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 30));
            c.handle(
                Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                &g,
                Default::default(),
            );
            assert_eq!(
                c.app.panes[0].selection_revision.wrapping_sub(revision),
                u64::from(changes),
                "{action:?}"
            );
            assert_eq!(c.preferences_changed, changes);
            assert!(c.app.policy_menu.is_none());
        }
    }
}
