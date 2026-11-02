//! Toasts and retained history share events, semantic styles and measured rectangles.
use super::modal;
use crate::{
    app::App,
    color::{Role, Theme},
    notifications::Placement,
};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Clear, Paragraph},
};

#[derive(Clone, Debug)]
pub struct Toast {
    pub area: Rect,
    pub event: usize,
    rows: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct HistoryLayout {
    pub shell: modal::Layout<crossterm::event::KeyCode>,
    pub rows: Vec<String>,
    pub limit: usize,
}

pub(super) fn history(app: &App, area: Rect) -> Option<HistoryLayout> {
    if !app.notifications.open {
        return None;
    }
    let outer = super::centered(
        Rect {
            height: area.height.saturating_sub(2),
            ..area
        },
        area.width.saturating_sub(2).min(96),
        area.height.saturating_sub(4).min(26),
    );
    use crossterm::event::KeyCode;
    let mut actions = vec![
        modal::actions::Action::custom("Up Older", KeyCode::Up, KeyCode::Up),
        modal::actions::Action::custom("Down Newer", KeyCode::Down, KeyCode::Down),
    ];
    if app
        .notifications
        .history
        .get(app.notifications.cursor)
        .is_some_and(|event| event.job.is_some())
    {
        actions.push(modal::actions::Action::custom(
            "Enter Jobs",
            KeyCode::Enter,
            KeyCode::Enter,
        ));
    }
    let spec = modal::Spec {
        title: "Notifications",
        help: false,
        content_width: 0,
        pinned_rows: 1,
        footer_rows: 1,
        actions: &actions,
    };
    let shell = modal::Layout::viewport(&spec, outer);
    let text = history_text(app);
    let rows = text
        .lines()
        .flat_map(|line| super::text::wrap_value(line, usize::from(shell.body.width)))
        .collect::<Vec<_>>();
    let limit = rows.len().saturating_sub(usize::from(shell.body.height));
    Some(HistoryLayout { shell, rows, limit })
}

fn history_text(app: &App) -> String {
    let Some(event) = app.notifications.history.get(app.notifications.cursor) else {
        return "No notifications this session.".into();
    };
    let time = crate::fuzzy_time::fuzzy_time(event.created, app.now);
    let route = event.job.map_or_else(String::new, |id| {
        format!("\nJob {id}: Enter opens Jobs evidence and recovery actions.")
    });
    format!(
        "{} · {} occurrences · {time}\n{}{}",
        event.severity.label(),
        event.occurrences,
        super::clean(&event.text),
        route
    )
}

pub(super) fn toasts(app: &App, geometry: &super::Geometry) -> Vec<Toast> {
    if app.notifications.open || app.popup {
        return Vec::new();
    }
    let top = geometry
        .panes
        .iter()
        .flatten()
        .map(|r| r.y)
        .min()
        .unwrap_or(geometry.status.y)
        .saturating_add(2);
    let available = Rect::new(
        geometry.status.x,
        top,
        geometry.status.width,
        geometry.status.y.saturating_sub(top + 2),
    );
    let mut result: Vec<Toast> = Vec::new();
    for id in app.notifications.visible() {
        if app.notifications.history[id]
            .job
            .is_some_and(|id| Some(id) == viewed_job(app))
        {
            continue;
        }
        let width = available.width.saturating_sub(2).min(60);
        if width < 16 || available.height < 3 {
            break;
        }
        let rows = toast_rows(app, id, width.saturating_sub(2));
        let height = rows.len() as u16 + 2;
        let Some(area) = place(
            app.notifications.placement,
            available,
            width,
            height,
            geometry,
            &result,
        ) else {
            continue;
        };
        result.push(Toast {
            area,
            event: id,
            rows,
        });
    }
    result
}

fn viewed_job(app: &App) -> Option<u64> {
    app.operations
        .show_history
        .then(|| {
            app.operations
                .history
                .get(app.operations.cursor)
                .map(|job| job.id)
        })
        .flatten()
}

fn place(
    placement: Placement,
    area: Rect,
    width: u16,
    height: u16,
    geometry: &super::Geometry,
    taken: &[Toast],
) -> Option<Rect> {
    if height > area.height {
        return None;
    }
    let left = area.x + 1;
    let right = area.right().saturating_sub(width + 1);
    let preferred_left = matches!(placement, Placement::TopLeft | Placement::BottomLeft);
    let top = matches!(placement, Placement::TopLeft | Placement::TopRight);
    for x in if preferred_left {
        [left, right]
    } else {
        [right, left]
    } {
        for offset in 0..=area.height - height {
            let y = if top {
                area.y + offset
            } else {
                area.bottom() - height - offset
            };
            let candidate = Rect::new(x, y, width, height);
            if !occupied(candidate, geometry) && taken.iter().all(|t| !candidate.intersects(t.area))
            {
                return Some(candidate);
            }
        }
    }
    None
}

fn occupied(area: Rect, g: &super::Geometry) -> bool {
    [
        g.menu,
        g.diagnostics,
        g.help.as_ref().map(|h| h.outer),
        g.pattern.as_ref().map(|p| p.shell.outer),
        g.operation_layout.as_ref().map(|l| l.shell.outer),
        g.context_help.as_ref().map(|h| h.shell.outer),
    ]
    .into_iter()
    .flatten()
    .any(|r| area.intersects(r))
}

fn toast_text(app: &App, id: usize) -> String {
    let event = &app.notifications.history[id];
    let key = if event.job.is_some() {
        app.shortcut(crate::input::Action::Jobs, "F9")
    } else {
        app.shortcut(crate::input::Action::Notifications, "Alt+h")
    };
    let repeat = if event.occurrences > 1 {
        format!(" (×{})", event.occurrences)
    } else {
        String::new()
    };
    format!(
        "{}: {}{repeat} — {key} details",
        event.severity.label(),
        super::clean(&event.text)
    )
}

fn toast_rows(app: &App, id: usize, width: u16) -> Vec<String> {
    let mut rows = super::text::wrap_value(&toast_text(app, id), usize::from(width));
    if rows.len() > 3 {
        rows.truncate(2);
        rows.push(modal::truncate(
            &format!(
                "… {} details",
                app.shortcut(crate::input::Action::Notifications, "Alt+h")
            ),
            width,
        ));
    }
    rows
}

pub(super) fn render(frame: &mut Frame, app: &App, g: &super::Geometry, theme: &Theme) {
    for toast in &g.toasts {
        let event = &app.notifications.history[toast.event];
        frame.render_widget(Clear, toast.area);
        let block =
            ratatui::widgets::Block::bordered().border_style(theme.style(event.severity.role()));
        frame.render_widget(
            Paragraph::new(toast.rows.join(
                "
",
            ))
            .block(block)
            .style(theme.style(event.severity.role())),
            toast.area,
        );
    }
    if let Some(layout) = &g.notification_history {
        render_history(frame, app, layout, theme);
    }
}

fn render_history(frame: &mut Frame, app: &App, layout: &HistoryLayout, theme: &Theme) {
    modal::render_measured(frame, &layout.shell, "Notifications", theme, Role::Header);
    modal::actions::render(frame, &layout.shell, theme, None);
    let state = &app.notifications;
    let header = format!(
        "{}/{} · Up/Down events · Home/End",
        if state.history.is_empty() {
            0
        } else {
            state.cursor + 1
        },
        state.history.len()
    );
    frame.render_widget(
        Paragraph::new(modal::truncate(&header, layout.shell.pinned.width))
            .style(theme.style(Role::Key)),
        layout.shell.pinned,
    );
    let rows: Vec<_> = layout
        .rows
        .iter()
        .skip(state.scroll.min(layout.limit))
        .take(usize::from(layout.shell.body.height))
        .cloned()
        .collect();
    let role = state
        .history
        .get(state.cursor)
        .map_or(Role::Value, |e| e.severity.role());
    frame.render_widget(
        Paragraph::new(rows.join("\n")).style(theme.style(role)),
        layout.shell.body,
    );
    frame.render_widget(
        Paragraph::new(modal::truncate(
            "PageUp/Down details · Esc close",
            layout.shell.footer.width,
        ))
        .style(theme.style(Role::Key)),
        layout.shell.footer,
    );
}
