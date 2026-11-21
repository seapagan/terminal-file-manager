//! One horizontal viewport and visible insertion/overwrite caret for single-line fields.
use crate::color::{Role, Theme};
use ratatui::{
    style::Modifier,
    text::{Line, Span},
};

pub(crate) fn value(
    text: &str,
    position: usize,
    overwrite: bool,
    focused: bool,
    width: u16,
    theme: &Theme,
) -> (Line<'static>, u16) {
    let position = position.min(text.len());
    let caret = if focused && width > 0 {
        if overwrite { "▐" } else { "▏" }
    } else {
        ""
    };
    let budget = width.saturating_sub(super::modal::width(caret));
    let mut start = position;
    let mut used = 0;
    for pair in crate::editor::boundaries(&text[..position])
        .windows(2)
        .rev()
    {
        let cells = super::modal::width(&text[pair[0]..pair[1]]);
        if used + cells > budget {
            break;
        }
        used += cells;
        start = pair[0];
    }
    let before = &text[start..position];
    let cursor = super::modal::width(before);
    let remaining = budget.saturating_sub(cursor);
    let after = super::modal::truncate(&text[position..], remaining);
    (
        Line::from(vec![
            Span::styled(before.to_owned(), theme.style(Role::Value)),
            Span::styled(
                caret,
                theme.style(Role::Header).add_modifier(Modifier::BOLD),
            ),
            Span::styled(after, theme.style(Role::Value)),
        ]),
        cursor,
    )
}

pub(crate) fn edited(
    text: &str,
    editor: &crate::editor::Editor,
    focused: bool,
    width: u16,
    theme: &Theme,
) -> (Line<'static>, u16) {
    let position = super::clean(&text[..editor.position(text)]).len();
    value(
        &super::clean(text),
        position,
        editor.overwrite,
        focused,
        width,
        theme,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_field_viewports_keep_the_caret_bounded() {
        let theme = Theme::new(colored_text::ColorLevel::NoColor);
        for size in [8192, 65_536] {
            check_viewport(size, &theme);
        }
    }
    fn check_viewport(size: usize, theme: &Theme) {
        let text = "x".repeat(size);
        let started = std::time::Instant::now();
        for position in [0, 1, size / 2, text.len()] {
            let (line, cursor) = value(&text, position, false, true, 20, theme);
            assert!(line.width() <= 20);
            assert!(cursor < 20);
            assert!(line.to_string().contains('▏'));
        }
        eprintln!(
            "{size}-byte field viewport samples: {:?}",
            started.elapsed()
        );
    }
}
