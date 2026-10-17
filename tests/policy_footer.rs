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
            "Custom │ Sort: Natural │ Group: First │ Hidden: Off │ Ignore: Dim"
        );
        assert_eq!(
            buffer[(10, 8)].fg == Color::Reset,
            level == ColorLevel::NoColor
        );
        assert_eq!(buffer[(16, 8)].fg, Color::Reset);
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
                    "Hidden: Off",
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
        assert!(!line.contains("hidden:") && !line.contains("//"));
    }
}
