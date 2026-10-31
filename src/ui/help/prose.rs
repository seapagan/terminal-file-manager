//! Word wrapping is local to Help; long tokens reuse display-cell wrapping.
use crate::{
    color::{Role, Theme},
    input::{HelpFragment, HelpParagraph},
};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(super) fn lines(
    paragraphs: &[HelpParagraph],
    width: u16,
    theme: Option<&Theme>,
) -> Vec<Line<'static>> {
    let mut rows = Vec::new();
    for paragraph in paragraphs {
        if !rows.is_empty() {
            rows.push(Line::default());
        }
        let spans: Vec<_> = paragraph
            .iter()
            .map(|fragment| {
                let (text, role) = match fragment {
                    HelpFragment::Text(text) => (text, Role::Value),
                    HelpFragment::Key(text) => (text, Role::Key),
                };
                Span::styled(
                    crate::ui::clean(text),
                    theme.map_or(Style::default(), |theme| theme.style(role)),
                )
            })
            .collect();
        rows.extend(wrap(Line::from(spans), usize::from(width)));
    }
    rows
}

fn wrap(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
    let mut rows = vec![Line::default()];
    if width == 0 {
        return rows;
    }
    let mut word = Line::default();
    for grapheme in line.styled_graphemes(Style::default()) {
        if grapheme.symbol.chars().all(char::is_whitespace) {
            append_word(&mut rows, &mut word, width);
        } else {
            word.push_span(Span::styled(grapheme.symbol.to_owned(), grapheme.style));
        }
    }
    append_word(&mut rows, &mut word, width);
    rows
}

fn append_word(rows: &mut Vec<Line<'static>>, word: &mut Line<'static>, width: usize) {
    if word.spans.is_empty() {
        return;
    }
    let word = std::mem::take(word);
    let used = rows.last().expect("row").width();
    if used > 0 && used + 1 + word.width() > width {
        rows.push(Line::default());
    }
    if word.width() > width {
        let mut parts = crate::ui::text::wrap_line(word, width).into_iter();
        *rows.last_mut().expect("row") = parts.next().expect("wrapped word");
        rows.extend(parts);
    } else {
        let row = rows.last_mut().expect("row");
        if row.width() > 0 {
            row.push_span(" ");
        }
        row.spans.extend(word.spans);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn strings(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(ToString::to_string).collect()
    }
    #[test]
    fn keeps_words_together_and_splits_only_tokens_wider_than_the_line() {
        assert_eq!(
            strings(&wrap(Line::raw("one two three"), 8)),
            ["one two", "three"]
        );
        assert_eq!(
            strings(&wrap(Line::raw("one abcdefghij end"), 5)),
            ["one", "abcde", "fghij", "end"]
        );
        assert_eq!(
            strings(&wrap(Line::raw(" alpha   beta "), 10)),
            ["alpha beta"]
        );
        assert_eq!(strings(&wrap(Line::raw("word"), 0)), [""]);
    }
    #[test]
    fn preserves_combining_wide_and_styled_graphemes_at_narrow_widths() {
        let key = Style::default().add_modifier(ratatui::style::Modifier::BOLD);
        let line = Line::from(vec![Span::raw("界e\u{301} "), Span::styled("Ctrl+界", key)]);
        let rows = wrap(line, 4);
        assert_eq!(strings(&rows), ["界e\u{301}", "Ctrl", "+界"]);
        assert!(rows.iter().all(|row| row.width() <= 4));
        assert!(
            rows[1..]
                .iter()
                .flat_map(|row| row.spans.iter())
                .all(|span| span.style == key)
        );
        assert_eq!(
            strings(&wrap(Line::raw("界e\u{301}"), 1)),
            ["�", "e\u{301}"]
        );
    }
}
