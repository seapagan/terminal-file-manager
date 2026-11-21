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
        let layout = layout(&app, bounds, &controls).unwrap();
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
