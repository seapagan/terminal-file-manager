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
    let mut start = 0;
    for end in crate::editor::boundaries(&text[..position]) {
        if super::modal::width(&text[end..position]) <= budget {
            start = end;
            break;
        }
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
