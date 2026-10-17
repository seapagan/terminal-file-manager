//! Display-cell wrapping shared by metadata and help.
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(super) fn wrap_value(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    let line = Line::raw(text);
    let mut rows = vec![String::new()];
    let mut used = 0;
    for grapheme in line.styled_graphemes(Style::default()) {
        let length = Span::raw(grapheme.symbol).width();
        if used + length > width && used > 0 {
            rows.push(String::new());
            used = 0;
        }
        // A wide grapheme cannot fit in a one-cell value column; preserve layout.
        if length > width {
            rows.last_mut().unwrap().push('�');
            used += 1;
        } else {
            rows.last_mut().unwrap().push_str(grapheme.symbol);
            used += length;
        }
    }
    rows
}
