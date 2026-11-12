use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Modifier};
use terminal_file_manager::{
    color::Theme,
    ui::workflow::{self, Control},
    workflow::Target,
};

#[test]
fn brackets_styles_and_hits_share_one_row_cells_at_every_width_and_colour() {
    for width in [0, 1, 2, 8, 24, 40, 65, 80, 100, 120, 160] {
        for level in [
            colored_text::ColorLevel::NoColor,
            colored_text::ColorLevel::Ansi16,
            colored_text::ColorLevel::Ansi256,
            colored_text::ColorLevel::TrueColor,
        ] {
            check_layout(width, level);
        }
    }
}

fn check_layout(width: u16, level: colored_text::ColorLevel) {
    let flow = super::flow();
    let area = Rect::new(3, 2, width, 1);
    let layout = workflow::layout(&flow, area);
    let mut terminal = Terminal::new(TestBackend::new(width + 6, 4)).unwrap();
    terminal
        .draw(|frame| workflow::render(frame, &layout, &Theme::new(level)))
        .unwrap();
    for control in &layout.controls {
        assert_eq!(control.area.height, 1);
        assert_eq!(control.area.y, area.y);
        assert!(control.area.x >= area.x && control.area.right() <= area.right());
        assert_eq!(
            usize::from(control.area.width),
            ratatui::text::Line::raw(&control.label).width()
        );
        assert_eq!(
            rendered_text(control, terminal.backend().buffer()),
            control.label
        );
        for x in control.area.x..control.area.right() {
            assert_eq!(layout.hit(x, area.y), control.target);
        }
        assert!(layout.hit(control.area.right(), area.y).is_none());
        if matches!(control.target, Some(Target::Choice(_))) {
            assert_choice(control, terminal.backend().buffer());
        }
    }
    for x in 0..width + 6 {
        assert!(layout.hit(x, 1).is_none());
        assert!(layout.hit(x, 3).is_none());
    }
}

fn rendered_text(control: &Control, buffer: &ratatui::buffer::Buffer) -> String {
    let mut text = String::new();
    let mut x = control.area.x;
    while x < control.area.right() {
        let symbol = buffer[(x, control.area.y)].symbol();
        text.push_str(symbol);
        x += u16::try_from(ratatui::text::Line::raw(symbol).width().max(1)).unwrap();
    }
    text
}

#[test]
fn unicode_choice_renders_wide_and_combining_glyphs_without_losing_label_text() {
    let mut flow = super::flow();
    flow.input(super::key(crossterm::event::KeyCode::Char('b')));
    flow.input(super::key(crossterm::event::KeyCode::Backspace));
    let layout = workflow::layout(&flow, Rect::new(0, 0, 160, 1));
    let choice = layout
        .controls
        .iter()
        .find(|c| c.target == Some(Target::Choice("second")))
        .unwrap();
    assert_eq!(choice.label, "[b Second界e\u{301}]");
    let mut terminal = Terminal::new(TestBackend::new(160, 1)).unwrap();
    terminal
        .draw(|frame| {
            workflow::render(
                frame,
                &layout,
                &Theme::new(colored_text::ColorLevel::NoColor),
            )
        })
        .unwrap();
    assert_eq!(
        rendered_text(choice, terminal.backend().buffer()),
        "[b Second界e\u{301}]"
    );
}

#[test]
fn navigation_space_is_reserved_before_optional_title_and_choices() {
    let mut flow = super::flow();
    flow.input(super::key(crossterm::event::KeyCode::Enter));
    for width in 3..=120 {
        let layout = workflow::layout(&flow, Rect::new(3, 2, width, 1));
        let cancel = layout
            .controls
            .iter()
            .find(|c| c.target == Some(Target::Cancel))
            .expect("Esc fits");
        assert_eq!(layout.hit(cancel.area.x, 2), Some(Target::Cancel));
        assert_eq!(
            layout
                .controls
                .iter()
                .any(|c| c.target == Some(Target::Back)),
            width >= 13
        );
    }
}

fn assert_choice(control: &Control, buffer: &ratatui::buffer::Buffer) {
    assert!(control.label.starts_with('[') && control.label.ends_with(']'));
    let left = &buffer[(control.area.x, control.area.y)];
    let middle = &buffer[(control.area.x + 1, control.area.y)];
    let right = &buffer[(control.area.right() - 1, control.area.y)];
    assert_eq!((left.symbol(), right.symbol()), ("[", "]"));
    if control.selected {
        for cell in [left, middle, right] {
            assert!(cell.modifier.contains(Modifier::BOLD | Modifier::REVERSED));
        }
    } else if !control.enabled {
        for x in control.area.x..control.area.right() {
            assert!(buffer[(x, control.area.y)].modifier.contains(Modifier::DIM));
        }
    } else {
        assert!(left.modifier.contains(Modifier::DIM));
        assert!(right.modifier.contains(Modifier::DIM));
        assert!(!middle.modifier.contains(Modifier::DIM));
    }
}

#[test]
fn disabled_mouse_choice_reports_its_reason_without_advancing_or_completing() {
    let mut flow = super::flow();
    let layout = workflow::layout(&flow, Rect::new(0, 0, 160, 1));
    let disabled = layout.controls.iter().find(|c| !c.enabled).unwrap();
    let target = layout.hit(disabled.area.x, disabled.area.y).unwrap();
    assert!(
        matches!(flow.input(terminal_file_manager::workflow::Input::Click(target)),
        terminal_file_manager::workflow::Progress::Disabled(reason) if reason == "Unavailable")
    );
    assert_eq!(flow.stage_index(), 0);
    assert_eq!(flow.highlighted().unwrap().id, "first");
    assert!(!flow.can_back());
}
