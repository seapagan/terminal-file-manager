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
pub(super) use layout::Layout;

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

pub(super) fn geometry(
    app: &App,
    terminal: Rect,
) -> (Vec<OperationControl>, usize, Option<Layout>) {
    if !visible(app) {
        return (Vec::new(), 0, None);
    }
    let mut layout = Layout::new(app, terminal);
    let mut controls = layout.buttons.clone();
    controls.extend(jobs::controls(app, layout.list));
    let rows = &layout.rows;
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
    let limit = layout.limit(rows.len());
    // Retain only the rows whose geometry was used for this frame's hitboxes.
    layout.rows = layout
        .rows
        .into_iter()
        .skip(scroll)
        .take(usize::from(layout.body.height))
        .collect();
    (controls, limit, Some(layout))
}

pub(super) fn render(frame: &mut Frame, app: &App, theme: &Theme, geometry: &super::Geometry) {
    notices(frame, app, theme, geometry.status);
    let Some(layout) = &geometry.operation_layout else {
        return;
    };
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
    let text: Vec<_> = layout
        .rows
        .iter()
        .map(|row| Line::styled(row.text.as_str(), theme.style(row.role)))
        .collect();
    frame.render_widget(Paragraph::new(text), layout.body);
    for control in &geometry.operation_controls {
        if matches!(control.target, OperationTarget::Key(_)) {
            frame.render_widget(
                Paragraph::new(format!("[{}]", control.label)).style(theme.style(Role::Key)),
                control.area,
            );
        }
    }
}

fn notices(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    if app.diagnostics
        || app.help
        || app.popup
        || app.view_menu.is_some()
        || app.profile_menu.is_some()
    {
        return;
    }
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
