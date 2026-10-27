use super::{
    content::Row,
    jobs::{outcome, stage_name, state, state_role},
};
use crate::{
    app::App,
    color::Role,
    operations::{Event, Job, JobState, Outcome},
};

pub(super) fn safety(app: &App) -> Vec<Row> {
    let Some(job) = app.operations.history.get(app.operations.cursor) else {
        return Vec::new();
    };
    let (message, action) = match job.state {
        JobState::Critical => (
            "CRITICAL: stop and report",
            "E evidence; acknowledgment cannot restore data",
        ),
        JobState::Unknown => (
            "UNKNOWN: safe exit/retry blocked",
            "Inspect E evidence; exit remains blocked",
        ),
        JobState::Partial => (
            "Partial result; sources may remain",
            "E evidence: inspect incomplete work before retry",
        ),
        JobState::Failed => (
            "Failed entries",
            "E evidence: correct the cause before retry",
        ),
        JobState::Skipped => ("Skipped entries", "E evidence: inspect before retry"),
        JobState::Cancelled => (
            "Cancelled; some work may have completed",
            "E evidence: inspect results and retained sources",
        ),
        _ => (
            "Up/Down jobs · PgUp/PgDown details",
            "evidence is session only",
        ),
    };
    vec![
        Row::styled(message, state_role(job.state)).single_line(),
        Row::styled(action, state_role(job.state)).single_line(),
    ]
}

pub(super) fn summary(app: &App, width: u16) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(error) = &app.operations.last_error {
        rows.push(Row::styled(
            format!("Latest request not started: {error}"),
            Role::Error,
        ));
    }
    let Some(job) = app.operations.history.get(app.operations.cursor) else {
        rows.push(Row::plain("No jobs this session."));
        return rows;
    };
    if let Some(record) = &job.record {
        let completed = record
            .entries
            .iter()
            .filter(|e| e.outcome == Outcome::Completed)
            .count();
        rows.push(Row::styled(
            format!(
                "{completed} {} {} · {}",
                if completed == 1 { "entry" } else { "entries" },
                completed_verb(job.plan.operation),
                state(job.state)
            ),
            state_role(job.state),
        ));
    } else {
        rows.push(Row::styled(state(job.state), state_role(job.state)));
    }
    let root = crate::ui::modal::common_root(job.plan.operands.iter().map(|e| e.path.as_path()));
    if let Some(root) = &root {
        rows.push(path_row("From", root, width));
    }
    if job.plan.operation != crate::operations::Operation::Trash
        && let Some(destination) = &job.plan.destination
    {
        rows.push(path_row("To", destination, width));
    }
    rows.extend(operand_rows(job, root.as_deref()));
    rows.extend(progress(job));
    rows
}

fn path_row(label: &str, path: &std::path::Path, width: u16) -> Row {
    Row::styled(
        format!(
            "{label}: {}",
            crate::ui::path_title::shorten(
                path,
                usize::from(width).saturating_sub(label.len() + 2)
            )
        ),
        Role::Label,
    )
    .single_line()
}

fn operand_rows(job: &Job, root: Option<&std::path::Path>) -> Vec<Row> {
    // Index recorded native identities once; descendants remain available in Evidence.
    let mut results = std::collections::HashMap::<_, Vec<_>>::new();
    if let Some(record) = &job.record {
        for result in &record.entries {
            results
                .entry(result.source.as_path())
                .or_default()
                .push(result);
        }
    }
    job.plan
        .operands
        .iter()
        .map(|operand| {
            let path = root
                .and_then(|r| operand.path.strip_prefix(r).ok())
                .unwrap_or(&operand.path);
            let mut row = crate::ui::modal::entry_row(operand, path);
            if let Some(entries) = results.get(operand.path.as_path()) {
                let last = entries.last().unwrap();
                row.role = match last.outcome {
                    Outcome::Completed => Role::Status,
                    Outcome::Failed | Outcome::SafetyIncident => Role::Error,
                    _ => Role::Warning,
                };
                if last.outcome != Outcome::Completed || entries.len() > 1 {
                    row.text = format!("{} · {} (E details)", outcome(last.outcome), row.text);
                }
            } else {
                row.text
                    .push_str(" · no recorded root completion (E details)");
                row.role = Role::Warning;
            }
            row
        })
        .collect()
}

fn completed_verb(operation: crate::operations::Operation) -> &'static str {
    use crate::operations::Operation::*;
    match operation {
        Trash => "trashed",
        Delete => "permanently deleted",
        Copy => "copied",
        Move => "moved",
        Rename => "renamed",
        Mkdir => "created",
        _ => "updated",
    }
}

fn progress(job: &Job) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(Event::Progress { stage, bytes, .. }) = &job.progress
        && job.active()
    {
        rows.push(Row::plain(format!(
            "Last observed: {} · {bytes} current-file bytes (not verified)",
            stage_name(*stage)
        )));
    }
    rows
}

pub(super) fn footer(
    app: &App,
    area: ratatui::layout::Rect,
    hidden: &[(&'static str, crossterm::event::KeyCode)],
) -> Vec<Row> {
    let width = area.width;
    let mut rows = safety(app);
    if width < 30
        && app
            .operations
            .history
            .get(app.operations.cursor)
            .is_some_and(|job| job.state == JobState::Unknown)
    {
        rows[0].text = "UNKNOWN: no R/exit".into();
    }
    if hidden.is_empty() {
        return super::content::wrapped(rows, width);
    }
    let warning = app
        .operations
        .history
        .get(app.operations.cursor)
        .is_some_and(|job| {
            !matches!(
                job.state,
                JobState::Completed | JobState::Running | JobState::Queued
            )
        });
    rows.truncate(usize::from(warning));
    rows.extend(shortcut_rows(
        hidden,
        width,
        usize::from(area.height).saturating_sub(usize::from(warning)),
    ));
    super::content::wrapped(rows, width)
}

fn shortcut_rows(
    actions: &[(&'static str, crossterm::event::KeyCode)],
    width: u16,
    capacity: usize,
) -> Vec<Row> {
    use crate::ui::modal::{truncate, width as cells};
    if capacity == 0 {
        return Vec::new();
    }
    let mut lines = Vec::<String>::new();
    for (label, _) in actions {
        if let Some(last) = lines.last_mut() {
            let joined = format!("{last} · {label}");
            if cells(&joined) <= width {
                *last = joined;
                continue;
            }
            if lines.len() == capacity {
                let last = lines.last_mut().unwrap();
                *last = truncate(&format!("{last} · …"), width);
                break;
            }
        }
        lines.push(truncate(label, width));
    }
    lines
        .into_iter()
        .map(|text| Row::styled(text, Role::Key).single_line())
        .collect()
}
