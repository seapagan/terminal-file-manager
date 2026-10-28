use super::*;

fn owner_continuation(c: &mut Controller, bounds: Rect, floating: bool) -> Rect {
    use terminal_file_manager::input::Action;
    let g = ui::geometry(&c.app, bounds);
    let limit = if floating {
        g.overlay_scroll_limit
    } else {
        g.metadata_scroll_limit
    };
    for scroll in 0..=limit {
        if floating {
            c.app.overlay_scroll = scroll;
        } else {
            c.app.metadata_scroll = scroll;
        }
        let controls: Vec<_> = ui::geometry(&c.app, bounds)
            .metadata_controls
            .into_iter()
            .filter(|control| control.action == Action::OwnerDisplay)
            .collect();
        if controls.len() > 1 {
            return controls[1].area;
        }
    }
    panic!("wrapped owner continuation");
}

#[test]
fn owner_values_and_continuations_exclude_labels_padding_and_borders() {
    use terminal_file_manager::{
        input::Action,
        owner::{OwnerDisplay, Ownership},
    };
    for (floating, width) in [(true, 24), (true, 40), (false, 120)] {
        let (_root, mut c) = loaded();
        c.app.popup = floating;
        c.app.details[0].entry.as_mut().unwrap().owner = Some(Ownership {
            uid: 12,
            gid: 34,
            user: "cached-user-".repeat(6),
            group: "cached-group".into(),
        });
        let bounds = Rect::new(0, 0, width, 12);
        let value = owner_continuation(&mut c, bounds, floating);
        let g = ui::geometry(&c.app, bounds);
        click(&mut c, value.x - 1, value.y, &g);
        click(&mut c, value.right(), value.y, &g);
        assert_eq!(
            c.app.metadata_format(floating).owner_display,
            OwnerDisplay::Names
        );
        click(&mut c, value.x, value.y, &g);
        assert_eq!(
            c.app.metadata_format(floating).owner_display,
            OwnerDisplay::Numeric
        );
        assert_eq!(c.app.panes[0].owner_display, OwnerDisplay::Names);
        assert_eq!(c.preferences.owner_display, OwnerDisplay::Names);
        assert!(!c.preferences_changed);
        assert!(
            g.metadata_controls
                .iter()
                .any(|control| control.action == Action::OwnerDisplay)
        );
    }
}
