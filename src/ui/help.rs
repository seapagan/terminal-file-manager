use crate::{
    app::App,
    color::{Role, Theme},
};
use ratatui::{
    Frame,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub(super) fn render(frame: &mut Frame, app: &App, theme: &Theme) {
    let (width, keys_width) = metrics(app, frame.area());
    let lines = rows(app, width.saturating_sub(2), keys_width, theme);
    let height = u16::try_from(lines.len() + 2)
        .unwrap_or(u16::MAX)
        .min(frame.area().height.saturating_sub(2));
    let area = super::centered(frame.area(), width as u16, height);
    let scroll = usize::from(app.overlay_scroll).min(
        lines
            .len()
            .saturating_sub(usize::from(height.saturating_sub(2))),
    ) as u16;
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).scroll((scroll, 0)).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Help — Esc closes; ↑/↓/wheel scroll")
                .border_style(theme.style(Role::Header)),
        ),
        area,
    );
}

fn rows(app: &App, width: usize, key_width: usize, theme: &Theme) -> Vec<Line<'static>> {
    app.help_rows
        .iter()
        .flat_map(|entry| {
            let (keys, action) = (&entry.keys, entry.description);
            let key_lines = super::text::wrap_value(keys, key_width);
            let action_lines = super::text::wrap_value(action, width.saturating_sub(key_width + 1));
            let count = key_lines.len().max(action_lines.len());
            (0..count).map(move |index| {
                let keys = key_lines.get(index).map_or("", String::as_str);
                let action_text = action_lines.get(index).cloned().unwrap_or_default();
                let padding = key_width.saturating_sub(Line::raw(keys).width()) + 1;
                Line::from(vec![
                    Span::styled(
                        format!("{keys}{}", " ".repeat(padding)),
                        theme.style(Role::Key),
                    ),
                    Span::styled(
                        action_text,
                        theme.style(if action.starts_with("Unavailable") {
                            Role::Disabled
                        } else {
                            Role::Value
                        }),
                    ),
                ])
            })
        })
        .collect()
}

fn metrics(app: &App, bounds: ratatui::layout::Rect) -> (usize, usize) {
    let natural_keys = app
        .help_rows
        .iter()
        .map(|entry| Line::raw(&entry.keys).width())
        .max()
        .unwrap_or(0);
    let natural_action = app
        .help_rows
        .iter()
        .map(|entry| Line::raw(entry.description).width())
        .max()
        .unwrap_or(0);
    let width = (natural_keys + natural_action + 5)
        .min(110)
        .min(usize::from(bounds.width.saturating_sub(2)));
    let keys_width = natural_keys.min(width.saturating_sub(3) / 3);
    (width, keys_width)
}

pub(super) fn scroll_limit(app: &App, bounds: ratatui::layout::Rect) -> u16 {
    let (width, key_width) = metrics(app, bounds);
    let count: usize = app
        .help_rows
        .iter()
        .map(|entry| {
            let (keys, action) = (&entry.keys, entry.description);
            super::text::wrap_value(keys, key_width)
                .len()
                .max(super::text::wrap_value(action, width.saturating_sub(key_width + 3)).len())
        })
        .sum();
    u16::try_from(count.saturating_sub(usize::from(bounds.height.saturating_sub(4))))
        .unwrap_or(u16::MAX)
}
