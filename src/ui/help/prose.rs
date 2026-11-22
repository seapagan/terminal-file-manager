//! Main and contextual Help share word wrapping and lossless grapheme splitting.
use crate::{color::Theme, input::HelpParagraph};
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
        rows.extend(wrap(line(paragraph, theme), usize::from(width)));
    }
    rows
}

pub(super) fn line(paragraph: &HelpParagraph, theme: Option<&Theme>) -> Line<'static> {
    Line::from(
        paragraph
            .iter()
            .map(|fragment| {
                Span::styled(
                    crate::ui::clean(fragment.text()),
                    theme.map_or(Style::default(), |theme| theme.style(fragment.role())),
                )
            })
            .collect::<Vec<_>>(),
    )
}

pub(super) fn truncate(line: Line<'static>, width: u16) -> Line<'static> {
    let plain = line.to_string();
    let clipped = crate::ui::modal::truncate(&plain, width);
    if clipped == plain {
        return line;
    }
    let mut bytes = clipped.strip_suffix('…').unwrap_or(&clipped).len();
    let mut result = Line::default();
    for grapheme in line.styled_graphemes(Style::default()) {
        if grapheme.symbol.len() > bytes {
            break;
        }
        result.push_span(Span::styled(grapheme.symbol.to_owned(), grapheme.style));
        bytes -= grapheme.symbol.len();
    }
    if width > 0 {
        result.push_span("…");
    }
    result
}

pub(super) fn wrap(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
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
        let mut parts = split_word(word, width).into_iter();
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

fn split_word(word: Line<'static>, width: usize) -> Vec<Line<'static>> {
    let mut rows = vec![Line::default()];
    let mut used = 0;
    for grapheme in word.styled_graphemes(Style::default()) {
        let cells = Span::raw(grapheme.symbol).width();
        if used > 0 && used + cells > width {
            rows.push(Line::default());
            used = 0;
        }
        // Keep indivisible graphemes in the document even if the viewport cannot show them.
        rows.last_mut()
            .expect("row")
            .push_span(Span::styled(grapheme.symbol.to_owned(), grapheme.style));
        used += cells;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    fn strings(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(ToString::to_string).collect()
    }
    #[test]
    fn semantic_truncation_matches_existing_display_cells_and_keeps_partial_styles() {
        let style = Style::default()
            .fg(ratatui::style::Color::Green)
            .add_modifier(ratatui::style::Modifier::BOLD);
        let source = "界e\u{301} OSC 52… end";
        for width in 0..20 {
            let result = truncate(Line::styled(source, style), width);
            assert_eq!(
                result.to_string(),
                crate::ui::modal::truncate(source, width)
            );
            assert!(
                result
                    .styled_graphemes(Style::default())
                    .filter(|grapheme| grapheme.symbol != "…")
                    .all(|grapheme| grapheme.style == style)
            );
        }
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
            ["界", "e\u{301}"]
        );
    }

    #[test]
    fn semantic_fragments_keep_styles_and_unicode_through_word_splitting() {
        use crate::{color::Role, input::HelpFragment};
        let theme = Theme::new(colored_text::ColorLevel::TrueColor);
        let paragraph = vec![
            HelpFragment::Reference("界e\u{301}OSC".into()),
            HelpFragment::Text(" ".into()),
            HelpFragment::Key("Alt+界".into()),
            HelpFragment::Text(" ".into()),
            HelpFragment::Label("‘名称’".into()),
        ];
        let rows = lines(&[paragraph], 4, Some(&theme));
        assert_eq!(
            strings(&rows),
            ["界e\u{301}O", "SC", "Alt+", "界", "‘名", "称’"]
        );
        let foregrounds: Vec<_> = rows.iter().map(|row| row.spans[0].style.fg).collect();
        assert_eq!(
            foregrounds,
            [
                Role::HelpReference,
                Role::HelpReference,
                Role::Key,
                Role::Key,
                Role::Value,
                Role::Value
            ]
            .map(|role| theme.style(role).fg)
        );
        assert!(rows.iter().all(|row| row.width() <= 4));
    }
}
