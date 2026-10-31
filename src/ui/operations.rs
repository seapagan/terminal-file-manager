use crate::{
    app::App,
    color::{Role, Theme},
};
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Clear, Paragraph},
};
mod actions;
mod conflict;
mod content;
mod jobs;
mod jobs_layout;
mod jobs_summary;
mod layout;
#[cfg(test)]
mod layout_tests;
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
    pub label: String,
    /// Projected shell eligibility for geometry consumers and tests.
    /// Activation uses the measured shell's hit/key policy, not this field.
    pub enabled: bool,
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
    controls.extend(field_controls(&layout.header, layout.shell.pinned, 0));
    controls.extend(field_controls(rows, layout.body, scroll));
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
    super::modal::render_measured(
        frame,
        &layout.shell,
        content::title(app),
        theme,
        content::title_role(app),
    );
    jobs::render(frame, app, theme, layout.list);
    render_rows(frame, &layout.header, layout.shell.pinned, theme);
    render_rows(frame, &layout.rows, layout.body, theme);
    render_footer(frame, app, layout, theme);
    if geometry.operation_scroll_limit > 0 && layout.shell.cue.height > 0 {
        let total = geometry.operation_scroll_limit + usize::from(layout.body.height);
        let start = app.operations.scroll.min(geometry.operation_scroll_limit);
        let cue = if layout.rows.is_empty() {
            format!("0/{total} · Resize")
        } else {
            format!(
                "{}-{}/{total} · PageUp/PageDown",
                start + 1,
                start + layout.rows.len()
            )
        };
        let area = layout.shell.cue;
        frame.render_widget(
            Paragraph::new(super::modal::truncate(&cue, area.width))
                .style(theme.style(Role::Label)),
            area,
        );
    }
    let selected = actions::focused(app).and_then(|key| {
        layout
            .shell
            .actions
            .iter()
            .find(|region| region.action.intent == OperationTarget::Key(key))
            .map(|region| region.index)
    });
    super::modal::actions::render(frame, &layout.shell, theme, selected);
}

fn render_footer(frame: &mut Frame, app: &App, layout: &Layout, theme: &Theme) {
    let footer_start = if app.operations.dialog.is_none()
        && (app.operations.show_history || !app.operations.conflicts.is_empty())
    {
        0
    } else {
        layout
            .footer
            .len()
            .saturating_sub(usize::from(layout.shell.footer.height))
    };
    render_rows(
        frame,
        &layout.footer[footer_start..],
        layout.shell.footer,
        theme,
    );
}

fn field_controls(rows: &[content::Row], area: Rect, scroll: usize) -> Vec<OperationControl> {
    rows.iter()
        .skip(scroll)
        .take(usize::from(area.height))
        .enumerate()
        .filter_map(|(offset, row)| {
            row.field.map(|field| OperationControl {
                area: Rect::new(area.x, area.y + offset as u16, area.width, 1),
                target: OperationTarget::Field(field),
                label: String::new(),
                enabled: true,
            })
        })
        .collect()
}

fn render_rows(frame: &mut Frame, rows: &[content::Row], area: Rect, theme: &Theme) {
    let text: Vec<_> = rows.iter().map(|row| row.line(area.width, theme)).collect();
    frame.render_widget(Paragraph::new(text), area);
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
            format!(
                "CRITICAL · {} details: OS reported permanent deletion during {} Trash",
                app.shortcut(crate::input::Action::Jobs, "F9"),
                app.shortcut(crate::input::Action::Trash, "F8")
            ),
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
            .map(|notice| short_notice(app, notice, area.width))
    };
    if let Some((text, role)) = notice {
        frame.render_widget(Clear, area);
        frame.render_widget(Paragraph::new(text).style(theme.style(role)), area);
    }
}

fn short_notice(app: &App, notice: &crate::operations::Notice, width: u16) -> (String, Role) {
    use crate::operations::NoticeSeverity;
    let (label, role) = match notice.severity {
        NoticeSeverity::Success => ("Success", Role::Status),
        NoticeSeverity::Warning => ("Warning", Role::Warning),
        NoticeSeverity::Error => ("Error", Role::Error),
    };
    let clean = super::clean(&notice.text);
    let summary = clean.strip_suffix(" — F9 details").unwrap_or(&clean);
    let text = format!("{label}: {summary}");
    (
        super::status_text(app, &format!("{text} — F9 details"), width),
        role,
    )
}

/// The event loop calls this with monotonic elapsed time; only visible pending counts redraw.
pub(crate) fn animate_preview(app: &mut App, elapsed: std::time::Duration) -> bool {
    let pending = visible(app)
        && matches!(&app.operations.dialog, Some(crate::operations::Dialog::Operation(d))
            if matches!(d.plan.operation, crate::operations::Operation::Trash | crate::operations::Operation::Delete)
                && d.plan.preview.is_none() && d.error.is_none());
    if !pending {
        app.operations.preview_frame = 0;
        return false;
    }
    let frame = ((elapsed.as_millis() / 150) % 4) as u8;
    let changed = frame != app.operations.preview_frame;
    app.operations.preview_frame = frame;
    changed
}
