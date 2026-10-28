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
    let summary = job.presentation();
    rows.push(Row::styled(heading(job, &summary), state_role(job.state)));
    rows.extend(result_rows(job, &summary));
    let root = crate::ui::modal::common_root(job.plan.operands.iter().map(|e| e.path.as_path()));
    if let Some(root) = &root {
        rows.push(path_row("From", root, width));
    }
    if job.plan.operation != crate::operations::Operation::Trash
        && let Some(destination) = &job.plan.destination
    {
        rows.push(path_row("To", destination, width));
    }
    rows.extend(operand_rows(job, &summary.roots, root.as_deref()));
    rows.extend(progress(job));
    rows
}

fn result_rows(job: &Job, summary: &crate::operations::JobSummary) -> Vec<Row> {
    if job.active() && job.record.is_some() {
        return vec![Row::styled(
            format!("Previous result (E Evidence): {}", summary.counts),
            Role::Warning,
        )];
    }
    let mut rows = Vec::new();
    rows.extend(summary.groups.iter().map(|(kind, detail, count)| {
        Row::styled(
            format!("{count} {}: {detail}", outcome(*kind).to_lowercase()),
            if matches!(
                kind,
                Outcome::Failed | Outcome::Changed | Outcome::SafetyIncident
            ) {
                Role::Error
            } else {
                Role::Warning
            },
        )
        .single_line()
    }));
    if summary.omitted_groups > 0 {
        rows.push(
            Row::styled(
                format!("{} more cause groups — E Evidence", summary.omitted_groups),
                Role::Warning,
            )
            .single_line(),
        );
    }
    rows
}

fn heading(job: &Job, summary: &crate::operations::JobSummary) -> String {
    if job.active() || job.record.is_none() {
        return super::jobs::phase(job);
    }
    if let Some(count) = summary.all_completed {
        format!(
            "{count} {} {} · {}",
            if count == 1 { "entry" } else { "entries" },
            completed_verb(job.plan.operation),
            state(job.state)
        )
    } else {
        format!("{} · {}", summary.counts, state(job.state))
    }
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

fn operand_rows(
    job: &Job,
    results: &[Option<Outcome>],
    root: Option<&std::path::Path>,
) -> Vec<Row> {
    job.plan
        .operands
        .iter()
        .zip(results)
        .map(|(operand, result)| {
            let path = root
                .and_then(|r| operand.path.strip_prefix(r).ok())
                .unwrap_or(&operand.path);
            let mut row = crate::ui::modal::entry_row(operand, path);
            // A retained record belongs to the previous result while reconciliation is active.
            let result = if job.active() { &None } else { result };
            let (cue, role) = match result {
                Some(Outcome::Completed) => ("Done", Role::Status),
                Some(Outcome::Failed) => ("Failed", Role::Error),
                Some(Outcome::SafetyIncident) => ("CRITICAL", Role::Error),
                Some(Outcome::Changed) => ("Changed", Role::Warning),
                Some(Outcome::Skipped) => ("Skipped", Role::Warning),
                Some(Outcome::Cancelled) => ("Cancel", Role::Warning),
                Some(Outcome::CopiedSourceRetained) => ("Retained", Role::Warning),
                Some(Outcome::OutcomeUnknown) => ("Unknown", Role::Warning),
                None if job.active() => (state(job.state), Role::Value),
                None => ("Unrecorded", Role::Warning),
            };
            row.role = role;
            row.file_status(cue)
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
