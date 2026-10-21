use crate::{
    app::App,
    color::{Role, Theme},
};
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};
mod actions;
mod content;
mod jobs;
mod layout;
use layout::Layout;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationTarget {
    Key(KeyCode),
    Job(usize),
    Field(usize),
}

#[derive(Clone, Debug)]
pub struct OperationControl {
    pub area: Rect,
    pub target: OperationTarget,
    pub label: &'static str,
}

pub(super) fn visible(app: &App) -> bool {
    !app.diagnostics
        && !app.help
        && !app.popup
        && app.view_menu.is_none()
        && app.profile_menu.is_none()
        && (app.operations.dialog.is_some()
            || app.operations.show_history
            || !app.operations.conflicts.is_empty()
            || app.operations.waiting_exit.is_some())
}

pub(super) fn controls(app: &App, terminal: Rect) -> Vec<OperationControl> {
    if !visible(app) {
        return Vec::new();
    }
    let layout = Layout::new(app, terminal);
    let mut controls = layout.buttons.clone();
    controls.extend(jobs::controls(app, layout.list));
    let rows = content::wrapped(app, layout.body.width);
    let scroll = app.operations.scroll.min(layout.limit(rows.len()));
    for (offset, row) in rows
        .iter()
        .skip(scroll)
        .take(usize::from(layout.body.height))
        .enumerate()
    {
        if let Some(field) = row.field {
            controls.push(OperationControl {
                area: Rect::new(
                    layout.body.x,
                    layout.body.y + offset as u16,
                    Line::raw(row.text.as_str())
                        .width()
                        .min(usize::from(layout.body.width)) as u16,
                    1,
                ),
                target: OperationTarget::Field(field),
                label: "",
            });
        }
    }
    controls
}

pub(super) fn scroll_limit(app: &App, terminal: Rect) -> u16 {
    if !visible(app) {
        return 0;
    }
    let layout = Layout::new(app, terminal);
    layout
        .limit(content::wrapped(app, layout.body.width).len())
        .min(u16::MAX as usize) as u16
}

pub(super) fn render(frame: &mut Frame, app: &App, theme: &Theme, controls: &[OperationControl]) {
    notices(frame, app, theme);
    if !visible(app) {
        return;
    }
    let layout = Layout::new(app, frame.area());
    frame.render_widget(Clear, layout.outer);
    frame.render_widget(
        Block::default()
            .title(content::title(app))
            .borders(Borders::ALL)
            .border_style(theme.style(Role::ActivePane))
            .style(theme.style(Role::Normal)),
        layout.outer,
    );
    jobs::render(frame, app, theme, layout.list);
    let rows = content::wrapped(app, layout.body.width);
    let scroll = app.operations.scroll.min(layout.limit(rows.len()));
    let text: Vec<_> = rows
        .into_iter()
        .map(|row| Line::styled(row.text, theme.style(row.role)))
        .collect();
    frame.render_widget(Paragraph::new(text).scroll((scroll as u16, 0)), layout.body);
    for control in controls {
        if matches!(control.target, OperationTarget::Key(_)) {
            frame.render_widget(
                Paragraph::new(format!("[{}]", control.label)).style(theme.style(Role::Key)),
                control.area,
            );
        }
    }
}

fn notices(frame: &mut Frame, app: &App, theme: &Theme) {
    if app.diagnostics
        || app.help
        || app.popup
        || app.view_menu.is_some()
        || app.profile_menu.is_some()
    {
        return;
    }
    let area = super::geometry(app, frame.area()).status;
    let notice = if app.operations.incidents() {
        Some((
            "CRITICAL · F9 details: OS reported permanent deletion during F8 Trash".to_owned(),
            Role::Error,
        ))
    } else if visible(app) {
        None
    } else {
        app.operations
            .notices
            .iter()
            .rev()
            .find(|notice| notice.expires > app.now)
            .map(|notice| short_notice(notice, area.width))
    };
    if let Some((text, role)) = notice {
        frame.render_widget(Clear, area);
        frame.render_widget(Paragraph::new(text).style(theme.style(role)), area);
    }
}

fn short_notice(notice: &crate::operations::Notice, width: u16) -> (String, Role) {
    use crate::operations::NoticeSeverity;
    let (label, role) = match notice.severity {
        NoticeSeverity::Success => ("Success", Role::Status),
        NoticeSeverity::Warning => ("Warning", Role::Warning),
        NoticeSeverity::Error => ("Error", Role::Error),
    };
    let clean = super::clean(&notice.text);
    let summary = clean.strip_suffix(" — F9 details").unwrap_or(&clean);
    let text = format!("{label}: {summary}");
    let summary = super::text::wrap_value(&text, usize::from(width.saturating_sub(13)).max(1))
        .into_iter()
        .next()
        .unwrap_or_default();
    (format!("{summary} — F9 details"), role)
}
