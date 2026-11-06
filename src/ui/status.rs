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

pub(super) type Control = (Rect, String, Role, Option<Action>);

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

pub(super) fn controls(app: &App, area: Rect) -> Vec<Control> {
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

fn critical_lines(app: &App, segment: Segment, area: Rect) -> Vec<Control> {
    let hint = format!(" · {} details", app.shortcut(Action::Jobs, "F9"));
    let summary = segment.full.replacen(&hint, "", 1);
    let mut lines = super::text::wrap_value(&summary, usize::from(area.width));
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

pub(super) fn render(frame: &mut Frame, controls: &[Control], theme: &Theme) {
    for (area, text, role, _) in controls {
        frame.render_widget(
            Paragraph::new(text.as_str()).style(theme.style(*role)),
            *area,
        );
    }
}

fn context(app: &App, width: u16) -> Segment {
    let text = super::status_text(app, &app.status, width);
    let severity = match app.status {
        crate::app::Status::Notice { severity, .. } => Some(severity),
        crate::app::Status::JobsDetails(_) => Some(crate::notifications::Severity::Error),
        _ => None,
    };
    Segment {
        compact: text.clone(),
        full: text,
        role: severity.map_or(Role::Status, crate::notifications::Severity::role),
        action: if matches!(app.status, crate::app::Status::JobsDetails(_)) {
            Some(Action::Jobs)
        } else {
            severity.map(|_| Action::Notifications)
        },
        priority: if matches!(app.status, crate::app::Status::JobsDetails(_))
            || severity.is_some_and(|severity| severity >= crate::notifications::Severity::Warning)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::Severity;

    #[test]
    fn wrapped_critical_rows_show_one_details_hint_and_keep_both_jobs_targets() {
        let app = App::new("owned-status".into());
        for (summary, label, role) in [
            (
                "CRITICAL · F9 details: OS reported permanent deletion during Trash",
                "CRITICAL",
                Role::Error,
            ),
            (
                "UNKNOWN: safe exit/retry blocked pending reconciliation · F9 details",
                "UNKNOWN",
                Role::Warning,
            ),
        ] {
            for width in [20, 40, 60] {
                let segment = Segment {
                    full: summary.into(),
                    compact: label.into(),
                    role,
                    action: Some(Action::Jobs),
                    priority: Priority::Critical,
                };
                let rows = critical_lines(&app, segment, Rect::new(2, 3, width, 2));
                let text = rows.iter().map(|row| row.1.as_str()).collect::<String>();
                assert!(text.starts_with(label), "{text}");
                assert_eq!(text.matches("F9 details").count(), 1, "{text}");
                assert!(rows[1].1.ends_with("F9 details"));
                assert!(rows.iter().all(|(area, _, _, action)| {
                    area.right() <= 2 + width && *action == Some(Action::Jobs)
                }));
                if width == 60 {
                    assert!(
                        text.contains("permanent deletion") || text.contains("exit/retry blocked")
                    );
                }
            }
        }
    }

    #[test]
    fn measured_status_segments_match_rendering_and_hit_regions_after_changes() {
        use ratatui::{Terminal, backend::TestBackend};

        let mut app = App::new("owned-status".into());
        let theme = Theme::new(colored_text::ColorLevel::NoColor);
        for (width, severity, message) in [
            (80, Severity::Error, "First failure"),
            (20, Severity::Warning, "Changed warning 界"),
            (120, Severity::Info, "Recovered"),
        ] {
            app.notify(severity, message);
            app.notifications.dismiss_toasts();
            let geometry = super::super::layout::geometry(&app, Rect::new(0, 0, width, 24));
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
            terminal
                .draw(|frame| {
                    super::super::render(frame, &app, &theme);
                })
                .unwrap();
            for (area, text, role, action) in &geometry.status_segments {
                let expected = Paragraph::new(text.as_str()).style(theme.style(*role));
                let mut buffer = ratatui::buffer::Buffer::empty(*area);
                ratatui::widgets::Widget::render(expected, *area, &mut buffer);
                for x in area.x..area.right() {
                    assert_eq!(
                        terminal.backend().buffer()[(x, area.y)],
                        buffer[(x, area.y)]
                    );
                }
                if let Some(action) = action {
                    assert!(geometry.status_controls.contains(&(*area, *action)));
                }
            }
        }
    }

    #[test]
    fn status_severity_survives_unrelated_events_and_resets_with_new_status() {
        let mut app = App::new("owned-status".into());
        app.notify(Severity::Error, "Current failure");
        app.notifications
            .notify("Watch recovered".into(), app.now, Severity::Info);
        let current = context(&app, 20);
        assert!(matches!(current.role, Role::Error));
        assert!(current.priority == Priority::Notice);
        assert_eq!(current.action, Some(Action::Notifications));
        app.status = "Current failure".into();
        let current = context(&app, 20);
        assert!(matches!(current.role, Role::Status));
        assert!(current.priority == Priority::Hint);
        assert_eq!(current.action, None);
    }
}
