use super::*;
use crate::ui::modal::{Spec, render_measured};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, widgets::Paragraph};

#[test]
fn field_width_measures_literal_display_cells_independently_of_focus() {
    for value in ["", "a filename longer than the title", "界e\u{301}🙂"] {
        let idle = Row::field("Name", value, (), false);
        let focused = Row::field("Name", value, (), true);
        assert_eq!(focused.width(), idle.width());
        assert_eq!(focused.width(), Line::raw(format!("Name: {value}")).width());
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
