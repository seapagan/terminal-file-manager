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
    style::Modifier,
    text::Line,
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
    pub start: usize,
}

pub(super) fn history(app: &App, area: Rect) -> Option<HistoryLayout> {
    if !app.notifications.open {
        return None;
    }
    let state = &app.notifications;
    let width = area.width.saturating_sub(2).min(88);
    let actions = history_actions(app);
    let spec = modal::Spec {
        title: "Notifications",
        help: false,
        content_width: usize::from(width.saturating_sub(4)),
        pinned_rows: 1,
        footer_rows: 0,
        actions: &actions,
    };
    let desired = if state.details {
        18
    } else {
        spec.measure(area, |_| state.history.len().clamp(1, 10))
            .outer
            .height
    };
    let outer = super::centered(area, width, area.height.saturating_sub(4).min(desired));
    let shell = modal::Layout::viewport(&spec, outer);
    let rows = if state.details {
        history_text(app)
            .lines()
            .flat_map(|line| super::text::wrap_value(line, usize::from(shell.body.width)))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let limit = rows.len().saturating_sub(usize::from(shell.body.height));
    let start = history_start(state.cursor, usize::from(shell.body.height));
    Some(HistoryLayout {
        shell,
        rows,
        limit,
        start,
    })
}

fn history_actions(app: &App) -> Vec<modal::actions::Action<crossterm::event::KeyCode>> {
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
    actions.push(modal::actions::Action::custom(
        if app.notifications.details {
            "e List"
        } else {
            "e Details"
        },
        KeyCode::Char('e'),
        KeyCode::Char('e'),
    ));
    actions
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

pub(super) fn toasts(app: &App, terminal: Rect) -> Vec<Toast> {
    if app.notifications.open {
        return Vec::new();
    }
    let width = terminal.width.saturating_sub(2).min(60);
    let budget = (u32::from(terminal.height) * 4 / 5) as u16;
    let budget = budget.min(terminal.height.saturating_sub(2));
    if width < 16 {
        return Vec::new();
    }
    let left = matches!(
        app.notifications.placement,
        Placement::TopLeft | Placement::BottomLeft
    );
    let top = matches!(
        app.notifications.placement,
        Placement::TopLeft | Placement::TopRight
    );
    let x = if left {
        terminal.x + 1
    } else {
        terminal.right().saturating_sub(width + 1)
    };
    let mut used = 0u16;
    let mut result = Vec::new();
    for id in app.notifications.visible() {
        let rows = toast_rows(app, id, width.saturating_sub(4));
        let height = rows.len() as u16 + 2;
        let spacing = u16::from(!result.is_empty());
        if used + spacing + height > budget {
            break;
        }
        used += spacing + height;
        let y = if top {
            terminal.y + 1 + used - height
        } else {
            terminal.bottom() - 1 - used
        };
        result.push(Toast {
            area: Rect::new(x, y, width, height),
            event: id,
            rows,
        });
    }
    result
}

fn toast_rows(app: &App, id: usize, width: u16) -> Vec<String> {
    let event = &app.notifications.history[id];
    let count = app.notifications.toast_occurrences(id);
    let repeat = if count > 1 {
        format!(" (×{count})")
    } else {
        String::new()
    };
    let title = if event.presentation.title == event.severity.label() {
        event.presentation.title.clone()
    } else {
        format!("{} · {}", event.severity.label(), event.presentation.title)
    };
    let mut rows = vec![format!(
        "{}{repeat}",
        modal::truncate(&title, width.saturating_sub(modal::width(&repeat)))
    )];
    let mut message = super::text::wrap_words(&event.presentation.message, usize::from(width));
    if message.len() > 2 {
        message.truncate(2);
        message[1] = modal::truncate(&format!("{} …", message[1]), width);
    }
    rows.extend(message);
    let key = if event.job.is_some() {
        app.shortcut(crate::input::Action::Jobs, "F9")
    } else {
        app.shortcut(crate::input::Action::Notifications, "Alt+h")
    };
    rows.push(modal::truncate(&format!("{key} details"), width));
    rows
}

pub(super) fn render(frame: &mut Frame, app: &App, g: &super::Geometry, theme: &Theme) {
    for toast in &g.toasts {
        let event = &app.notifications.history[toast.event];
        frame.render_widget(Clear, toast.area);
        let surface = theme.notification_style();
        frame.render_widget(Paragraph::new("").style(surface), toast.area);
        for y in toast.area.y..toast.area.bottom() {
            frame.buffer_mut()[(toast.area.x, y)]
                .set_symbol("▌")
                .set_style(surface.patch(theme.style(event.severity.role())));
        }
        let last = toast.rows.len().saturating_sub(1);
        let lines: Vec<_> = toast
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let style = if index == 0 {
                    theme
                        .style(event.severity.role())
                        .add_modifier(Modifier::BOLD)
                } else if index == last {
                    surface.add_modifier(Modifier::ITALIC)
                } else {
                    theme.style(Role::Normal)
                };
                Line::styled(row.clone(), surface.patch(style))
            })
            .collect();
        frame.render_widget(
            Paragraph::new(lines).style(surface),
            Rect::new(
                toast.area.x + 2,
                toast.area.y + 1,
                toast.area.width.saturating_sub(4),
                toast.area.height.saturating_sub(2),
            ),
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
    let controls = if state.details {
        "PgUp/PgDown scroll · Up/Down events · Esc close"
    } else {
        "Up/Down · PgUp/PgDown · Home/End · Esc close"
    };
    let header = format!(
        "{}/{} · {controls}",
        usize::from(!state.history.is_empty()) + state.cursor,
        state.history.len()
    );
    frame.render_widget(
        Paragraph::new(modal::truncate(&header, layout.shell.pinned.width))
            .style(theme.style(Role::Key)),
        layout.shell.pinned,
    );
    if state.details {
        let rows = layout
            .rows
            .iter()
            .skip(state.scroll.min(layout.limit))
            .take(usize::from(layout.shell.body.height))
            .cloned()
            .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(rows.join("\n")).style(theme.style(Role::Value)),
            layout.shell.body,
        );
    } else {
        render_list(frame, app, layout, theme);
    }
}

fn render_list(frame: &mut Frame, app: &App, layout: &HistoryLayout, theme: &Theme) {
    let state = &app.notifications;
    let height = usize::from(layout.shell.body.height);
    let start = layout.start;
    if state.history.is_empty() {
        frame.render_widget(
            Paragraph::new("No notifications this session."),
            layout.shell.body,
        );
    }
    for (row, (id, event)) in state
        .history
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .enumerate()
    {
        let selected = id == state.cursor;
        let width = layout.shell.body.width;
        let line = history_row(event, selected, width, app.now);
        let style = if selected {
            theme.style(Role::Cursor)
        } else {
            theme.style(event.severity.role())
        };
        frame.render_widget(
            Paragraph::new(line).style(style),
            Rect::new(
                layout.shell.body.x,
                layout.shell.body.y + row as u16,
                width,
                1,
            ),
        );
    }
}

pub(crate) fn history_start(cursor: usize, height: usize) -> usize {
    cursor.saturating_sub(height.saturating_sub(1))
}

fn history_row(
    event: &crate::notifications::Notice,
    selected: bool,
    width: u16,
    now: std::time::SystemTime,
) -> String {
    let marker = if selected { ">" } else { " " };
    let time = history_age(event.created, now);
    let repeat = if event.occurrences > 1 {
        format!(" ×{}", event.occurrences)
    } else {
        String::new()
    };
    let time_width = (width / 4).clamp(3, 5);
    let time = modal::truncate(&time, time_width);
    let suffix = format!("  {time:>column$}", column = usize::from(time_width));
    let summary = if event.presentation.title == event.severity.label() {
        event.presentation.message.clone()
    } else {
        format!(
            "{}: {}",
            event.presentation.title, event.presentation.message
        )
    };
    let summary = format!("{marker} {}{repeat} {summary}", event.severity.label());
    let summary = summary.replace('\n', " ");
    if width > modal::width(&suffix) {
        let available = width - modal::width(&suffix);
        let clipped = modal::truncate(&super::clean(&summary), available);
        format!(
            "{clipped}{}{suffix}",
            " ".repeat(usize::from(
                available.saturating_sub(modal::width(&clipped))
            ))
        )
    } else {
        modal::truncate(&super::clean(&summary), width)
    }
}

fn history_age(created: std::time::SystemTime, now: std::time::SystemTime) -> String {
    let (age, future) = now
        .duration_since(created)
        .map_or_else(|error| (error.duration(), true), |age| (age, false));
    let seconds = age.as_secs();
    let (amount, unit) = match seconds {
        0 => return "now".into(),
        1..60 => (seconds, "s"),
        60..3600 => (seconds / 60, "m"),
        3600..86400 => (seconds / 3600, "h"),
        _ => (seconds / 86400, "d"),
    };
    format!("{}{amount}{unit}", if future { "in " } else { "" })
}
