//! Display-cell wrapping shared by metadata and help.
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(super) fn wrap_value(text: &str, width: usize) -> Vec<String> {
    wrap_line(Line::raw(text.to_owned()), width)
        .into_iter()
        .map(|line| line.spans.into_iter().map(|span| span.content).collect())
        .collect()
}

pub(super) fn wrap_line(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::default()];
    }
    let mut rows = vec![Line::default()];
    let mut used = 0;
    for grapheme in line.styled_graphemes(Style::default()) {
        let length = Span::raw(grapheme.symbol).width();
        if used + length > width && used > 0 {
            rows.push(Line::default());
            used = 0;
        }
        // A wide grapheme cannot fit in a one-cell value column; preserve layout.
        if length > width {
            rows.last_mut()
                .unwrap()
                .push_span(Span::styled("�", grapheme.style));
            used += 1;
        } else {
            rows.last_mut()
                .unwrap()
                .push_span(Span::styled(grapheme.symbol.to_owned(), grapheme.style));
            used += length;
        }
    }
    rows
}
