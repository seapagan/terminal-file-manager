//! Display-cell wrapping shared by metadata and help.
use ratatui::{
    style::Style,
    text::{Line, Span},
};

/// Preserve words and intentional lines; elide an overlong token rather than split it.
pub(super) fn wrap_words(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    let mut rows = Vec::new();
    for line in text.lines() {
        let mut row = String::new();
        for word in line.split_whitespace() {
            let word =
                super::modal::truncate(&super::clean(word), width.min(u16::MAX as usize) as u16);
            let separator = usize::from(!row.is_empty());
            if !row.is_empty()
                && Line::raw(&row).width() + separator + Line::raw(&word).width() > width
            {
                rows.push(std::mem::take(&mut row));
            }
            if !row.is_empty() {
                row.push(' ');
            }
            row.push_str(&word);
        }
        rows.push(row);
    }
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

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
    fn notification_wrapping_preserves_words_lines_and_elides_oversized_tokens() {
        assert_eq!(
            wrap_words("Permission denied for file\nTry another folder", 18),
            ["Permission denied", "for file", "Try another folder"]
        );
        assert_eq!(
            wrap_words("界e\u{301} xy abcdefgh", 4),
            ["界e\u{301}", "xy", "abc…"]
        );
    }

    #[test]
    fn plain_wrapping_keeps_wide_and_combining_graphemes_with_bounded_cells() {
        assert_eq!(wrap_value("界e\u{301}xy", 3), ["界e\u{301}", "xy"]);
        assert_eq!(wrap_value("界e\u{301}xy", 1), ["�", "e\u{301}", "x", "y"]);
        assert_eq!(wrap_value("abc", 0), [""]);
        assert_eq!(wrap_value("", 4), [""]);
        assert_eq!(wrap_value("abcdef", 3), ["abc", "def"]);
    }
}
