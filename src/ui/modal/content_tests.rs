use super::*;
use crate::ui::modal::{Spec, render_measured};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, widgets::Paragraph};

#[test]
fn field_width_measures_literal_display_cells_independently_of_focus() {
    for value in ["", "a filename longer than the title", "界e\u{301}🙂"] {
        let editor = crate::editor::Editor::default();
        let idle = Row::field("Name", value, (), false).editor(&editor);
        let focused = Row::field("Name", value, (), true).editor(&editor);
        assert_eq!(focused.width(), idle.width());
        assert_eq!(
            focused.width(),
            Line::raw(format!("Name: {value}")).width() + 1
        );
    }
}

#[test]
fn changing_field_focus_preserves_dialog_geometry_and_literal_buffer() {
    for (width, height) in [(80, 24), (40, 18), (24, 14)] {
        let bounds = Rect::new(0, 0, width, height);
        let first = rendered_fields(bounds, true);
        let second = rendered_fields(bounds, false);
        assert_eq!(first, second, "{bounds:?}");
        assert_eq!(first.0.intersection(bounds), first.0);
        assert!(first.2.iter().any(|row| row.contains("Edit fields")));
    }
}

fn rendered_fields(bounds: Rect, first_focused: bool) -> (Rect, Rect, Vec<String>) {
    let rows = [
        Row::field("Name", "界e\u{301}🙂-a-long-filename", 0, first_focused),
        Row::field("Other", "short", 1, !first_focused),
    ];
    let layout = Spec::<()> {
        title: "Edit fields",
        help: false,
        content_width: rows.iter().map(Row::width).max().unwrap(),
        pinned_rows: 0,
        footer_rows: 0,
        actions: &[],
    }
    .measure(bounds, |width| {
        rows.iter().cloned().flat_map(|row| row.wrap(width)).count()
    });
    let theme = Theme::new(colored_text::ColorLevel::NoColor);
    let lines: Vec<_> = rows
        .into_iter()
        .flat_map(|row| row.wrap(layout.body.width))
        .map(|row| row.line(layout.body.width, &theme))
        .collect();
    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    terminal
        .draw(|frame| {
            render_measured(frame, &layout, "Edit fields", &theme, Role::Header);
            frame.render_widget(Paragraph::new(lines), layout.body);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let text = (0..bounds.height)
        .map(|y| (0..bounds.width).map(|x| buffer[(x, y)].symbol()).collect())
        .collect();
    (layout.outer, layout.body, text)
}

#[test]
fn exact_fit_native_preview_keeps_its_text_when_focus_changes() {
    let value = "\"native/long-destination\\xFF\"";
    let theme = Theme::new(colored_text::ColorLevel::NoColor);
    for focused in [false, true] {
        let row = Row::field("To", value, (), focused).placeholder();
        let width = row.width() as u16;
        assert_eq!(row.line(width, &theme).to_string(), format!("To: {value}"));
        assert_eq!(row.cursor(width), focused.then_some((4, false)));
    }
}

#[test]
fn exact_fit_editors_render_the_whole_value_and_end_cursor_with_stable_geometry() {
    for (label, value) in [
        ("Name", "an-exact-fit-filename.txt"),
        ("To", "directory/nested/destination"),
        ("To", "directory/界e\u{301}🙂/destination"),
    ] {
        for width in [80, 40, 24] {
            let mut previous = None;
            for focused in [false, true] {
                let row =
                    Row::field(label, value, (), focused).editor(&crate::editor::Editor::default());
                let bounds = Rect::new(0, 0, width, 24);
                let (body, text) = rendered_editor(&row, bounds);
                if width == 80 {
                    let expected = format!("{label}: {value}")
                        .replace("界", "界 ")
                        .replace("🙂", "🙂 ");
                    assert!(text.contains(&expected), "{text}");
                    assert_eq!(
                        row.cursor(body.width),
                        focused.then_some((body.width - 1, false))
                    );
                } else {
                    assert!(
                        text.contains("destination") || text.contains(".txt"),
                        "{text}"
                    );
                    assert!(row.cursor(body.width).is_none_or(|(x, _)| x < body.width));
                }
                if let Some(before) = previous {
                    assert_eq!(before, (body, text.clone()));
                }
                previous = Some((body, text));
            }
        }
    }
}

fn rendered_editor(row: &Row<()>, bounds: Rect) -> (Rect, String) {
    let layout = Spec::<()> {
        title: "Edit",
        help: false,
        content_width: row.width(),
        pinned_rows: 0,
        footer_rows: 0,
        actions: &[],
    }
    .measure(bounds, |width| row.clone().wrap(width).len());
    let theme = Theme::new(colored_text::ColorLevel::NoColor);
    let lines: Vec<_> = row
        .clone()
        .wrap(layout.body.width)
        .into_iter()
        .map(|row| row.line(layout.body.width, &theme))
        .collect();
    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    terminal
        .draw(|frame| {
            render_measured(frame, &layout, "Edit", &theme, Role::Header);
            frame.render_widget(Paragraph::new(lines), layout.body);
        })
        .unwrap();
    assert_eq!(layout.outer.intersection(bounds), layout.outer);
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    (layout.body, text)
}
