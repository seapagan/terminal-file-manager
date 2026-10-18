//! Policy fields fit as complete labelled segments instead of a debug sentence.
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Color};
use terminal_file_manager::{app::App, color::Theme, ui};

fn draw(app: &App, width: u16, level: ColorLevel) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(level));
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn policy(buffer: &Buffer, width: u16) -> String {
    (1..width.saturating_sub(1))
        .map(|x| buffer[(x, 8)].symbol())
        .collect::<String>()
        .trim_end()
        .into()
}

#[test]
fn policy_uses_labels_and_semantic_styles_without_changing_selection_summary() {
    let app = App::new(".".into());
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let buffer = draw(&app, 120, level);
        assert_eq!(
            policy(&buffer, 120),
            "Custom │ Sort: Natural │ Group: First │ Hidden: Hide │ Ignore: Dim"
        );
        assert_eq!(
            buffer[(10, 8)].fg == Color::Reset,
            level == ColorLevel::NoColor
        );
        assert_eq!(
            buffer[(16, 8)].fg,
            Theme::new(level)
                .style(terminal_file_manager::color::Role::ActivePane)
                .fg
                .unwrap_or(Color::Reset)
        );
        let summary = (1..119)
            .map(|x| buffer[(x, 7)].symbol())
            .collect::<String>();
        assert!(summary.contains("0 items  │  No selection"));
    }
}

#[test]
fn narrow_policy_omits_whole_segments_and_retains_reverse_filter_when_they_fit() {
    let mut app = App::new(".".into());
    app.panes[0].sort.reverse = true;
    app.panes[0].filter = "abc".into();
    for width in [3, 8, 20, 40, 80, 120] {
        let buffer = draw(&app, width, ColorLevel::NoColor);
        let line = policy(&buffer, width);
        for segment in line.split(" │ ").filter(|s| !s.is_empty()) {
            assert!(
                [
                    "Custom",
                    "Sort: Natural",
                    "Group: First",
                    "Hidden: Hide",
                    "Ignore: Dim",
                    "Reverse: On",
                    "Filter: abc"
                ]
                .contains(&segment),
                "{width}: {segment}"
            );
        }
        if width >= 80 {
            assert!(line.contains("Reverse: On") && line.contains("Filter: abc"));
        }
    }
}

#[test]
fn footer_info_uses_the_popup_binding_and_fits_whole_controls_in_both_layouts() {
    use crossterm::event::KeyEvent;
    use terminal_file_manager::input::{Action, Bindings, chord};
    let bindings = Bindings::new(&Default::default()).unwrap();
    let (code, mods) = chord("Alt+Enter").unwrap();
    assert_eq!(
        bindings.action(KeyEvent::new(code, mods)),
        Some(Action::Metadata)
    );
    let (code, mods) = chord("Alt+i").unwrap();
    assert_eq!(
        bindings.action(KeyEvent::new(code, mods)),
        Some(Action::ToggleSidebar)
    );
    for dual in [false, true] {
        for width in [1, 20, 60, 80, 120] {
            let mut app = App::new(".".into());
            app.dual = dual;
            let buffer = draw(&app, width, ColorLevel::NoColor);
            let g = ui::geometry(&app, ratatui::layout::Rect::new(0, 0, width, 12));
            assert!(
                g.footer_controls
                    .iter()
                    .all(|c| c.area.intersection(g.footer) == c.area)
            );
            let info = g
                .footer_controls
                .iter()
                .find(|c| c.action == Action::Metadata);
            if width >= 80 {
                let info = info.unwrap();
                assert_eq!(info.label, "Alt+Enter Info");
                let value: String = (info.area.x..info.area.right())
                    .map(|x| buffer[(x, g.footer.y)].symbol())
                    .collect();
                assert_eq!(value, info.label);
                click_info(app, &g, info);
            }
            assert!(
                g.footer_controls
                    .iter()
                    .all(|c| c.action != Action::ToggleSidebar)
            );
        }
    }
}

fn click_info(app: App, g: &ui::Geometry, info: &ui::FooterControl) {
    use terminal_file_manager::input::Bindings;
    let mut c = terminal_file_manager::input::Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    c.handle(
        crossterm::event::Event::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: info.area.x,
            row: info.area.y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }),
        g,
        std::time::Duration::ZERO,
    );
    assert!(c.app.popup && !c.app.sidebar);
}

#[test]
fn hidden_policy_labels_match_both_visibility_states() {
    let mut app = App::new(".".into());
    for (shown, expected) in [(false, "Hidden: Hide"), (true, "Hidden: Show")] {
        app.panes[0].show_hidden = shown;
        for level in [
            ColorLevel::NoColor,
            ColorLevel::Ansi16,
            ColorLevel::Ansi256,
            ColorLevel::TrueColor,
        ] {
            assert!(policy(&draw(&app, 120, level), 120).contains(expected));
        }
    }
}
