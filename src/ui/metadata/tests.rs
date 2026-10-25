use super::*;
use crate::ui::text::wrap_value;
use colored_text::ColorLevel;

#[test]
fn wrapped_values_keep_graphemes_and_continue_under_value_column() {
    let theme = Theme::new(ColorLevel::Ansi16);
    let fields = vec![Field::plain(
        "Path",
        "界e\u{301}界e\u{301}界e\u{301}\u{1b}\n".into(),
        &theme,
    )];
    let lines: Vec<_> = rows(&fields, 13, &theme)
        .into_iter()
        .map(|(line, _, _)| line)
        .collect();
    assert!(lines.len() > 1);
    let mut restored = String::new();
    for (index, line) in lines.iter().enumerate() {
        assert!(line.width() <= 13);
        assert_eq!(line.spans[0].width(), 6);
        assert_eq!(line.spans[0].content.trim().is_empty(), index != 0);
        for span in &line.spans[1..] {
            assert!(!span.content.contains('\u{1b}'));
            restored.push_str(&span.content);
        }
    }
    assert_eq!(restored, "界e\u{301}界e\u{301}界e\u{301}\\u{1b}\\n");
    assert!(
        lines[0].spans[0]
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
    assert_ne!(lines[0].spans[0].style.fg, lines[0].spans[1].style.fg);
}

#[test]
fn one_cell_values_do_not_overflow_or_split_combining_sequences() {
    assert_eq!(wrap_value("界e\u{301}", 1), ["�", "e\u{301}"]);
    assert_eq!(wrap_value("", 1), [""]);
    assert_eq!(wrap_value("value", 0), [""]);
}
pub(super) fn metadata_text(app: &App, floating: bool) -> String {
    use ratatui::{Terminal, backend::TestBackend};
    let width = if floating { 180 } else { 48 };
    let bounds = Rect::new(0, 0, width, 40);
    let theme = Theme::new(ColorLevel::NoColor);
    let area = layout(app, bounds, &theme, floating).area;
    let mut terminal = Terminal::new(TestBackend::new(width, 40)).unwrap();
    terminal
        .draw(|frame| render(frame, app, bounds, &theme, floating))
        .unwrap();
    let buffer = terminal.backend().buffer();
    // Rejoin wrapped metadata values without borders or continuation padding.
    (area.y + 1..area.bottom() - 1)
        .map(|y| {
            (area.x + 1..area.right() - 1)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim()
                .to_owned()
        })
        .collect()
}

#[test]
fn keys_help_describes_escape_for_the_rendered_metadata_view() {
    use ratatui::{Terminal, backend::TestBackend};
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(root.path().into());
    let theme = Theme::new(ColorLevel::NoColor);
    for (floating, expected) in [
        (false, "c: count/size on demand; Esc cancels the count"),
        (true, "c: count/size on demand; Esc cancels/closes"),
    ] {
        for counting in [false, true] {
            app.details[0].counting = counting;
            let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
            terminal
                .draw(|frame| render(frame, &app, Rect::new(0, 0, 100, 40), &theme, floating))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let row = (0..40)
                .map(|y| {
                    (0..100)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .find(|row| row.contains("Keys:"))
                .expect("visible metadata Keys row");
            assert_eq!(
                row.split("Keys:")
                    .nth(1)
                    .unwrap()
                    .trim()
                    .trim_end_matches('│')
                    .trim(),
                expected
            );
        }
    }
}

#[test]
fn totals_views_distinguish_running_complete_incomplete_and_failed_results() {
    for floating in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mut app = App::new(root.path().into());
        app.details[0].entry = Some(crate::fs::inspect(root.path()));
        app.details[0].path = Some(root.path().into());
        app.popup = floating;
        app.sidebar = !floating;
        app.details[0].counting = true;
        assert!(metadata_text(&app, floating).contains("Calculating totals"));
        app.details[0].counting = false;
        app.details[0].totals = Some(Ok(crate::fs::Totals {
            files: 1,
            directories: 2,
            bytes: 3,
            errors: vec![],
        }));
        let complete = metadata_text(&app, floating);
        assert!(complete.contains("1 files, 2 directories, 3 B"));
        assert!(!complete.contains("INCOMPLETE") && !complete.contains("Calculating totals"));
        app.details[0]
            .totals
            .as_mut()
            .unwrap()
            .as_mut()
            .unwrap()
            .errors
            .push("unreadable descendant".into());
        let incomplete = metadata_text(&app, floating);
        assert!(incomplete.contains("1 files, 2 directories, 3 B"));
        assert!(incomplete.contains("INCOMPLETE") && incomplete.contains("unreadable descendant"));
        app.details[0].totals = Some(Err("root unavailable".into()));
        let failed = metadata_text(&app, floating);
        assert!(failed.contains("root unavailable"));
        assert!(!failed.contains("Descendants") && !failed.contains("INCOMPLETE"));
    }
}
