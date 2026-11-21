use super::*;

pub(super) fn open(c: &mut Controller, action: Action, bounds: Rect) -> Rect {
    let g = ui::geometry(&c.app, bounds);
    let control = g
        .policy_controls
        .iter()
        .find(|c| c.action == action)
        .unwrap();
    let anchor = control.area;
    click(c, anchor.x, anchor.y, bounds);
    anchor
}

#[test]
fn narrow_dual_pane_popups_stay_in_the_owner_and_activate_its_choice() {
    let bounds = Rect::new(0, 0, 80, 24);
    for pane in 0..2 {
        let mut c = controller();
        c.app.dual = true;
        c.app.split_percent = if pane == 0 { 20 } else { 80 };
        let g = ui::geometry(&c.app, bounds);
        let owner = g.panes[pane].unwrap();
        let anchor = g
            .policy_controls
            .iter()
            .find(|control| control.pane == pane && control.action == Action::Sort)
            .unwrap()
            .area;
        click(&mut c, anchor.x, anchor.y, bounds);
        key(&mut c, "End", bounds);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut g = ui::geometry(&c.app, bounds);
        terminal
            .draw(|frame| {
                g = ui::render(
                    frame,
                    &c.app,
                    &Theme::new(colored_text::ColorLevel::NoColor),
                );
            })
            .unwrap();
        let popup = g.policy_menu.unwrap();
        assert_eq!(popup.shell.outer.intersection(owner), popup.shell.outer);
        assert_choice_rows(terminal.backend().buffer(), popup.shell.body, "Unsorted");
        click(
            &mut c,
            popup.shell.body.x,
            popup.shell.body.bottom() - 1,
            bounds,
        );
        assert_eq!(c.app.panes[pane].sort.sort, Sort::Unsorted);
        assert_eq!(c.app.panes[1 - pane].sort.sort, Sort::Natural);
    }
}

#[test]
fn all_policy_popups_render_only_choice_rows_between_the_borders() {
    let bounds = Rect::new(0, 0, 120, 30);
    for (action, count, last) in [
        (Action::Sort, 9, "Unsorted"),
        (Action::Group, 3, "Mixed"),
        (Action::Hidden, 2, "Show"),
        (Action::Ignore, 3, "Off"),
    ] {
        let mut c = controller();
        let anchor = open(&mut c, action, bounds);
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let mut g = ui::geometry(&c.app, bounds);
        terminal
            .draw(|f| g = ui::render(f, &c.app, &Theme::new(colored_text::ColorLevel::NoColor)))
            .unwrap();
        let layout = g.policy_menu.unwrap();
        assert_eq!(layout.shell.outer.height, count + 2, "{action:?}");
        assert_eq!(layout.shell.outer.bottom(), anchor.y);
        assert_eq!(layout.shell.body.y, layout.shell.outer.y + 1);
        assert_eq!(layout.shell.body.bottom(), layout.shell.outer.bottom() - 1);
        assert_choice_borders(terminal.backend().buffer(), &layout.shell);
        assert_choice_rows(terminal.backend().buffer(), layout.shell.body, last);
        click(
            &mut c,
            layout.shell.body.x,
            layout.shell.body.bottom() - 1,
            bounds,
        );
        assert!(c.app.policy_menu.is_none());
        let selected = match action {
            Action::Sort => c.app.panes[0].sort.sort == Sort::Unsorted,
            Action::Group => c.app.panes[0].sort.grouping == Grouping::Mixed,
            Action::Hidden => c.app.panes[0].show_hidden,
            Action::Ignore => c.app.panes[0].ignore == terminal_file_manager::app::IgnoreMode::Off,
            _ => unreachable!(),
        };
        assert!(selected, "{action:?}");
    }
}

#[test]
fn resized_sort_popup_keeps_the_scrolled_last_row_clickable() {
    let mut c = controller();
    let large = Rect::new(0, 0, 120, 30);
    open(&mut c, Action::Sort, large);
    key(&mut c, "End", large);
    let small = Rect::new(0, 0, 120, 9);
    let g = ui::geometry(&c.app, small);
    c.handle(
        Event::Resize(small.width, small.height),
        &g,
        Default::default(),
    );
    let g = ui::geometry(&c.app, small);
    let layout = g.policy_menu.unwrap();
    assert_eq!(layout.shell.outer.intersection(small), layout.shell.outer);
    assert!(layout.start > 0);
    let mut terminal = Terminal::new(TestBackend::new(small.right(), small.bottom())).unwrap();
    terminal
        .draw(|frame| {
            ui::render(
                frame,
                &c.app,
                &Theme::new(colored_text::ColorLevel::NoColor),
            );
        })
        .unwrap();
    assert_choice_rows(terminal.backend().buffer(), layout.shell.body, "Unsorted");
    click(
        &mut c,
        layout.shell.body.x,
        layout.shell.body.bottom() - 1,
        small,
    );
    assert_eq!(c.app.panes[0].sort.sort, Sort::Unsorted);
    assert!(c.app.policy_menu.is_none());
}

fn assert_choice_rows(buffer: &ratatui::buffer::Buffer, body: Rect, last: &str) {
    for y in body.y..body.bottom() {
        let row: String = (body.x..body.right())
            .map(|x| buffer[(x, y)].symbol())
            .collect();
        assert!(
            row.starts_with("[x] ") || row.starts_with("[ ] "),
            "{row:?}"
        );
    }
    let row: String = (body.x..body.right())
        .map(|x| buffer[(x, body.bottom() - 1)].symbol())
        .collect();
    assert!(row.contains(last), "{row}");
}

fn assert_choice_borders(buffer: &ratatui::buffer::Buffer, shell: &ui::modal::Layout<()>) {
    let outer = shell.outer;
    for (y, left, right) in [(outer.y, "┌", "┐"), (outer.bottom() - 1, "└", "┘")] {
        assert_eq!(buffer[(outer.x, y)].symbol(), left);
        assert_eq!(buffer[(outer.right() - 1, y)].symbol(), right);
        for x in outer.x + 1..outer.right() - 1 {
            if !shell.help.is_some_and(|help| help.contains((x, y).into())) {
                assert_eq!(buffer[(x, y)].symbol(), "─", "({x}, {y})");
            }
        }
    }
}

#[test]
fn headerless_choices_keep_keyboard_highlighting_and_outside_dismissal() {
    let bounds = Rect::new(0, 0, 120, 30);
    for action in [Action::Sort, Action::Group, Action::Hidden, Action::Ignore] {
        let mut c = controller();
        open(&mut c, action, bounds);
        key(&mut c, "Down", bounds);
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let mut g = ui::geometry(&c.app, bounds);
        terminal
            .draw(|f| g = ui::render(f, &c.app, &Theme::new(colored_text::ColorLevel::NoColor)))
            .unwrap();
        let body = g.policy_menu.unwrap().shell.body;
        assert!(
            terminal.backend().buffer()[(body.x, body.y + 1)]
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        );
        click(&mut c, 0, 0, bounds);
        assert!(c.app.policy_menu.is_none());
        assert!(!c.preferences_changed);
    }
}
