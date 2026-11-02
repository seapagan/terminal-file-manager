//! One responsive owner budgets persistent application status and its hit regions.
use crate::{
    app::App,
    color::{Role, Theme},
    input::Action,
};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Priority {
    Critical,
    Notice,
    Progress,
    Selection,
    History,
    Hint,
}

struct Segment {
    full: String,
    compact: String,
    role: Role,
    action: Option<Action>,
    priority: Priority,
}

fn critical(app: &App) -> Option<Segment> {
    let (text, compact, role) = if app.operations.incidents() {
        (
            "OS reported permanent deletion during Trash",
            "CRITICAL",
            Role::Error,
        )
    } else if app.operations.unknown() {
        (
            "UNKNOWN: safe exit/retry blocked pending reconciliation",
            "UNKNOWN: exit blocked",
            Role::Warning,
        )
    } else {
        return None;
    };
    let key = app.shortcut(Action::Jobs, "F9");
    Some(Segment {
        full: if app.operations.incidents() {
            format!("CRITICAL · {key} details: {text}")
        } else {
            format!("{text} · {key} details")
        },
        compact: format!("{compact} · {key}"),
        role,
        action: Some(Action::Jobs),
        priority: Priority::Critical,
    })
}

pub(super) fn rows(app: &App, area: Rect) -> u16 {
    if area.height < 2 {
        return 0;
    }
    let modal = crate::input::HelpTopic::current(app).is_some() || app.help || app.diagnostics;
    if !modal
        && area.height >= 18
        && critical(app).is_some_and(|s| super::modal::width(&s.full) > area.width)
    {
        2
    } else {
        1
    }
}

fn segments(app: &App, width: u16) -> Vec<Segment> {
    let mut items: Vec<_> = critical(app).into_iter().collect();
    let active = app
        .operations
        .history
        .iter()
        .filter(|job| job.active())
        .count();
    if active > 0 {
        let key = app.shortcut(Action::Jobs, "F9");
        items.push(Segment {
            full: format!("Jobs {active} active · {key}"),
            compact: format!("Jobs {active}"),
            role: Role::Value,
            action: Some(Action::Jobs),
            priority: Priority::Progress,
        });
    }
    items.push(context(app, width));
    items.extend(selection(app.panes[app.active].selection_counts()));
    if !app.notifications.history.is_empty() {
        items.push(Segment {
            full: format!(
                "{} history ({})",
                app.shortcut(Action::Notifications, "Alt+h"),
                app.notifications.history.len()
            ),
            compact: format!("{} history", app.shortcut(Action::Notifications, "Alt+h")),
            role: Role::Key,
            action: Some(Action::Notifications),
            priority: Priority::History,
        });
    }
    items.sort_by_key(|s| s.priority);
    items
}

pub(super) fn controls(app: &App, area: Rect) -> Vec<(Rect, String, Role, Option<Action>)> {
    let mut result = Vec::new();
    let mut x = area.x;
    let y = area.y;
    for segment in segments(app, area.width) {
        let remaining = area.right().saturating_sub(x);
        if remaining == 0 || area.height == 0 {
            break;
        }
        if segment.priority == Priority::Critical && area.height == 2 {
            return critical_lines(app, segment, area);
        }
        let text = if segment.priority == Priority::Notice
            && matches!(app.status, crate::app::Status::JobsDetails(_))
        {
            super::details_text(app, &app.status, remaining)
        } else if super::modal::width(&segment.full) <= remaining {
            segment.full
        } else {
            segment.compact
        };
        let text = super::modal::truncate(&text, remaining);
        let width = super::modal::width(&text);
        result.push((
            Rect::new(x, y, width, 1),
            text,
            segment.role,
            segment.action,
        ));
        x = x.saturating_add(width).saturating_add(3).min(area.right());
    }
    result
}

fn critical_lines(
    app: &App,
    segment: Segment,
    area: Rect,
) -> Vec<(Rect, String, Role, Option<Action>)> {
    let mut lines = super::text::wrap_value(&segment.full, usize::from(area.width));
    let first = lines.remove(0);
    let second = super::details_text(app, &lines.concat(), area.width);
    [first, second]
        .into_iter()
        .enumerate()
        .map(|(row, text)| {
            (
                Rect::new(area.x, area.y + row as u16, super::modal::width(&text), 1),
                text,
                segment.role,
                segment.action,
            )
        })
        .collect()
}

pub(super) fn render(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    for (area, text, role, _) in controls(app, area) {
        frame.render_widget(Paragraph::new(text).style(theme.style(role)), area);
    }
}

fn context(app: &App, width: u16) -> Segment {
    let text = super::status_text(app, &app.status, width);
    let event = app
        .notifications
        .latest_index()
        .and_then(|index| app.notifications.history.get(index))
        .filter(|event| event.text == *app.status);
    Segment {
        compact: text.clone(),
        full: text,
        role: event.map_or(Role::Status, |event| event.severity.role()),
        action: if matches!(app.status, crate::app::Status::JobsDetails(_)) {
            Some(Action::Jobs)
        } else {
            event.map(|_| Action::Notifications)
        },
        priority: if matches!(app.status, crate::app::Status::JobsDetails(_))
            || event.is_some_and(|event| event.severity >= crate::notifications::Severity::Warning)
        {
            Priority::Notice
        } else {
            Priority::Hint
        },
    }
}

fn selection(count: crate::app::SelectionCounts) -> Option<Segment> {
    if count.visible + count.excluded() > 0 {
        let extra = if count.excluded() > 0 {
            format!(" +{} excluded", count.excluded())
        } else {
            String::new()
        };
        let text = format!("Selected {}{extra}", count.visible);
        return Some(Segment {
            full: text,
            compact: format!("{} sel{extra}", count.visible),
            role: Role::Value,
            action: None,
            priority: Priority::Selection,
        });
    }
    None
}
