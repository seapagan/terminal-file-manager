use super::*;

#[test]
fn menu_width_fits_its_longest_rendered_choice_with_equal_padding() {
    let bounds = Rect::new(0, 0, 120, 30);
    for (action, longest) in [
        (Action::Sort, "Extension"),
        (Action::Group, "First"),
        (Action::Hidden, "Show"),
        (Action::Ignore, "Hide"),
    ] {
        let mut c = controller();
        let anchor = compact::open(&mut c, action, bounds);
        let g = ui::geometry(&c.app, bounds);
        let layout = g.policy_menu.unwrap();
        let text = format!("[ ] {longest}");
        let cells = ratatui::text::Line::raw(&text).width() as u16;
        assert_eq!(layout.shell.body.width, cells);
        let outer = layout.shell.outer;
        let body = layout.shell.body;
        assert_eq!(outer.intersection(bounds), outer);
        assert_eq!(body.intersection(outer), body);
        let left = body.x - outer.x;
        let right = outer.right() - body.right();
        assert!(left > 0, "choices must clear the border");
        assert_eq!(left, right, "{action:?}: asymmetric padding");
        assert_eq!(layout.shell.outer.bottom(), anchor.y);
    }
}
