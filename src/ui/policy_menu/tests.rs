use super::*;
use crate::input::{Action, policy_menu::PolicyMenu};

#[test]
fn choice_measurement_counts_markers_and_unicode_display_cells() {
    for label in ["界界界", "e\u{301}", "🙂", "Extension"] {
        let expected = super::super::modal::width(&format!("[x] {label}"));
        assert_eq!(choice_width(&["x", label]), expected.max(5));
        assert_eq!(choice_text(label, false).chars().next(), Some('['));
    }
}

#[test]
fn constrained_choice_geometry_stays_inside_offset_terminal_bounds() {
    let mut app = App::new("owned".into());
    app.policy_menu = Some(PolicyMenu {
        pane: 0,
        action: Action::Sort,
        selected: 8,
    });
    for width in 1..=20 {
        let bounds = Rect::new(3, 5, width, 9);
        let controls = [PolicyControl {
            pane: 0,
            action: Action::Sort,
            area: Rect::new(bounds.right() - 1, bounds.bottom() - 1, 1, 1),
        }];
        let layout = layout(&app, bounds, &controls, &[Some(bounds), None]).unwrap();
        assert_eq!(layout.shell.outer.width, width.min(17));
        assert_eq!(layout.shell.outer.intersection(bounds), layout.shell.outer);
        if let Some(help) = layout.shell.help {
            assert!(width >= 5);
            assert_eq!(help.intersection(layout.shell.outer), help);
            assert_eq!(help.right(), layout.shell.outer.right() - 1);
        } else {
            assert!(width < 5);
        }
        assert_eq!(layout.start + usize::from(layout.shell.body.height), 9);
    }
}

#[test]
fn choice_popups_clamp_to_each_owning_pane_even_when_content_is_wider() {
    let mut app = App::new("owned".into());
    for pane in 0..2 {
        app.policy_menu = Some(PolicyMenu {
            pane,
            action: Action::Sort,
            selected: 8,
        });
        for width in [8, 24, 40] {
            let bounds = Rect::new(3, 5, width * 2, 20);
            let panes = [
                Some(Rect::new(3, 5, width, 20)),
                Some(Rect::new(3 + width, 5, width, 20)),
            ];
            let owner = panes[pane].unwrap();
            let controls = [PolicyControl {
                pane,
                action: Action::Sort,
                area: Rect::new(owner.right() - 2, 23, 1, 1),
            }];
            let popup = layout(&app, bounds, &controls, &panes).unwrap();
            assert_eq!(popup.shell.outer.intersection(owner), popup.shell.outer);
            assert_eq!(popup.shell.outer.width, width.min(17));
            assert_eq!(popup.shell.outer.bottom(), 23);
        }
    }
}

#[test]
fn missing_controls_anchor_inside_each_offset_pane_even_at_constrained_heights() {
    let mut app = App::new("owned".into());
    for pane in 0..2 {
        app.policy_menu = Some(PolicyMenu {
            pane,
            action: Action::Sort,
            selected: 8,
        });
        for width in [1, 8, 24, 40] {
            for height in [1, 3, 9] {
                let bounds = Rect::new(3, 5, width * 2, height);
                let panes = [
                    Some(Rect::new(3, 5, width, height)),
                    Some(Rect::new(3 + width, 5, width, height)),
                ];
                let owner = panes[pane].unwrap();
                let popup = layout(&app, bounds, &[], &panes).unwrap();
                assert_eq!(popup.shell.outer.intersection(owner), popup.shell.outer);
                assert_eq!(popup.shell.outer.width, width.min(17));
                assert_eq!(popup.shell.outer.x, owner.x);
            }
        }
    }
}
