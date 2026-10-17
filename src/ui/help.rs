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
    let natural_keys = app
        .help_rows
        .iter()
        .map(|(keys, _)| Line::raw(keys).width())
        .max()
        .unwrap_or(0);
    let natural_action = app
        .help_rows
        .iter()
        .map(|(_, action)| Line::raw(action).width())
        .max()
        .unwrap_or(0);
    let width = (natural_keys + natural_action + 5)
        .min(110)
        .min(usize::from(frame.area().width.saturating_sub(2)));
    let keys_width = natural_keys.min(width.saturating_sub(3) / 3);
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
        .flat_map(|(keys, action)| {
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
