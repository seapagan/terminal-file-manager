//! Text-only horizontal viewports and a real cursor owned by the topmost input.
use crate::color::{Role, Theme};
use ratatui::{
    layout::{Position, Rect},
    text::Line,
};

pub(crate) struct Cursor {
    pub position: Position,
    pub overwrite: bool,
}

pub(crate) fn value(
    text: &str,
    position: usize,
    width: u16,
    theme: &Theme,
) -> (Line<'static>, u16) {
    let position = position.min(text.len());
    let input =
        tui_input::Input::new(text.to_owned()).with_cursor(text[..position].chars().count());
    let scroll = input.visual_scroll(usize::from(width.saturating_sub(1)));
    let mut start = 0;
    let mut columns = 0;
    for grapheme in Line::raw(text).styled_graphemes(ratatui::style::Style::default()) {
        if columns >= scroll {
            break;
        }
        columns += usize::from(super::modal::width(grapheme.symbol));
        start += grapheme.symbol.len();
    }
    let mut visible = String::new();
    let mut used = 0;
    for grapheme in Line::raw(&text[start..]).styled_graphemes(ratatui::style::Style::default()) {
        used += super::modal::width(grapheme.symbol);
        if used > width {
            break;
        }
        visible.push_str(grapheme.symbol);
    }
    let line = Line::styled(visible, theme.style(Role::Value));
    (line, input.visual_cursor().saturating_sub(scroll) as u16)
}

pub(crate) fn edited(
    text: &str,
    editor: &crate::editor::Editor,
    width: u16,
    theme: &Theme,
) -> (Line<'static>, u16) {
    let position = super::clean(&text[..editor.position(text)]).len();
    value(&super::clean(text), position, width, theme)
}

pub(crate) fn field_cursor(
    text: &str,
    editor: &crate::editor::Editor,
    label: u16,
    area: Rect,
) -> Option<Cursor> {
    let width = area.width.saturating_sub(label);
    if width == 0 || area.height == 0 {
        return None;
    }
    let (_, column) = edited(
        text,
        editor,
        width,
        &Theme::new(colored_text::ColorLevel::NoColor),
    );
    Some(Cursor {
        position: Position::new(area.x + label + column.min(width - 1), area.y),
        overwrite: editor.overwrite,
    })
}

pub(crate) fn cursor(app: &crate::app::App, g: &super::Geometry) -> Option<Cursor> {
    if app.notifications.open || g.context_help.is_some() || g.diagnostics.is_some() {
        return None;
    }
    let cursor = owner_cursor(app, g)?;
    (!g.toasts
        .iter()
        .any(|toast| toast.area.contains(cursor.position)))
    .then_some(cursor)
}

fn owner_cursor(app: &crate::app::App, g: &super::Geometry) -> Option<Cursor> {
    if g.operation_layout.is_some() {
        return super::operations::cursor(g);
    }
    if let Some(layout) = &g.pattern {
        return super::pattern::cursor(layout);
    }
    if g.path_picker.is_some() {
        return None;
    }
    if let Some(shell) = &g.menu_layout {
        return super::profiles::cursor(app, shell);
    }
    if let Some(help) = &g.help {
        return super::help::cursor(app, help);
    }
    if app.popup || g.policy_menu.is_some() {
        return None;
    }
    filter_cursor(app, g)
}

fn filter_cursor(app: &crate::app::App, g: &super::Geometry) -> Option<Cursor> {
    let pane = &app.panes[app.active];
    if pane.error.is_some() || !pane.warnings.is_empty() {
        return None;
    }
    let editor = pane.filter_editor.as_ref()?;
    let area = g.panes[app.active]?;
    if area.height < 3 {
        return None;
    }
    let area = Rect::new(
        area.x + 1,
        area.bottom() - 2,
        area.width.saturating_sub(2),
        1,
    );
    let label = super::modal::width(&super::modal::truncate("FILTER: ", area.width));
    field_cursor(&pane.filter, editor, label, area)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_motion_does_not_insert_or_displace_text() {
        let theme = Theme::new(colored_text::ColorLevel::NoColor);
        for position in [0, 1, 3] {
            let (line, cursor) = value("abc", position, 20, &theme);
            assert_eq!(line.to_string(), "abc");
            assert_eq!(cursor, position as u16);
        }
    }
    #[test]
    fn long_field_viewports_keep_the_real_cursor_bounded() {
        let theme = Theme::new(colored_text::ColorLevel::NoColor);
        for size in [8192, 65_536] {
            let text = "x".repeat(size);
            for position in [0, 1, size / 2, size] {
                let (line, cursor) = value(&text, position, 20, &theme);
                assert!(line.width() <= 20);
                assert!(cursor < 20);
            }
        }
    }
}
