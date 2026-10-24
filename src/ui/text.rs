//! Display-cell wrapping shared by metadata and help.
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(super) fn wrap_value(text: &str, width: usize) -> Vec<String> {
    if width == 0 || text.is_empty() {
        return vec![String::new()];
    }
    if text.bytes().all(|byte| (b' '..=b'~').contains(&byte)) {
        return text
            .as_bytes()
            .chunks(width)
            .map(|chunk| std::str::from_utf8(chunk).unwrap().to_owned())
            .collect();
    }
    let capacity = width.min(text.len());
    let mut rows = vec![String::with_capacity(capacity)];
    let mut used = 0;
    let line = Line::raw(text);
    for grapheme in line.styled_graphemes(Style::default()) {
        let length = Span::raw(grapheme.symbol).width();
        if used + length > width && used > 0 {
            rows.push(String::with_capacity(capacity));
            used = 0;
        }
        let (symbol, length) = if length > width {
            ("�", 1)
        } else {
            (grapheme.symbol, length)
        };
        rows.last_mut().unwrap().push_str(symbol);
        used += length;
    }
    rows
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_wrapping_keeps_wide_and_combining_graphemes_with_bounded_cells() {
        assert_eq!(wrap_value("界e\u{301}xy", 3), ["界e\u{301}", "xy"]);
        assert_eq!(wrap_value("界e\u{301}xy", 1), ["�", "e\u{301}", "x", "y"]);
        assert_eq!(wrap_value("abc", 0), [""]);
        assert_eq!(wrap_value("", 4), [""]);
        assert_eq!(wrap_value("abcdef", 3), ["abc", "def"]);
    }
}
