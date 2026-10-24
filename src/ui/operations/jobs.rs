use super::{
    OperationControl, OperationTarget,
    content::{self, Row},
};
use crate::{
    app::App,
    color::{Role, Theme},
    operations::{Event, Job, JobState, Outcome, Stage},
};
use ratatui::{Frame, layout::Rect, text::Line, widgets::Paragraph};

fn start(app: &App, area: Rect) -> usize {
    app.operations
        .cursor
        .saturating_sub(usize::from(area.height) / 2)
        .min(
            app.operations
                .history
                .len()
                .saturating_sub(usize::from(area.height)),
        )
}

pub(super) fn controls(app: &App, area: Rect) -> Vec<OperationControl> {
    app.operations
        .history
        .iter()
        .enumerate()
        .skip(start(app, area))
        .take(usize::from(area.height))
        .map(|(index, _)| OperationControl {
            area: Rect::new(
                area.x,
                area.y + (index - start(app, area)) as u16,
                area.width,
                1,
            ),
            target: OperationTarget::Job(index),
            label: "",
        })
        .collect()
}

pub(super) fn render(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let rows: Vec<_> = app
        .operations
        .history
        .iter()
        .enumerate()
        .skip(start(app, area))
        .take(usize::from(area.height))
        .map(|(index, job)| {
            Line::styled(
                format!(
                    "{} #{} {} · {}",
                    if index == app.operations.cursor {
                        ">"
                    } else {
                        " "
                    },
                    job.id,
                    content::operation(job.plan.operation),
                    state(job.state)
                ),
                theme.style(if index == app.operations.cursor {
                    Role::Cursor
                } else {
                    state_role(job.state)
                }),
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(rows), area);
}

pub(super) fn details(app: &App) -> Vec<Row> {
    let mut rows = vec![Row::styled(
        "Up/Down selects · PgUp/PgDown scroll details · evidence is session only",
        Role::Label,
    )];
    if let Some(error) = &app.operations.last_error {
        rows.push(Row::styled("Latest request not started", Role::Error));
        rows.push(Row::styled(error, Role::Error));
    }
    let Some(job) = app.operations.history.get(app.operations.cursor) else {
        rows.push(Row::plain("No jobs this session."));
        return rows;
    };
    rows.push(Row::styled(
        format!(
            "Job {}: {} · {}",
            job.id,
            content::operation(job.plan.operation),
            state(job.state)
        ),
        state_role(job.state),
    ));
    rows.push(Row::plain(crate::operations::summary(job)));
    rows.extend(route(app, job));
    if let Some(Event::Progress {
        source,
        stage,
        bytes,
        ..
    }) = &job.progress
    {
        rows.push(Row::plain(format!(
            "Last observed stage: {} · {} · {bytes} current-file bytes observed",
            stage_name(*stage),
            source.display()
        )));
    }
    rows.extend(disabled(job));
    if let Some(record) = &job.record {
        rows.push(Row::plain(format!("Observed bytes: {}", record.bytes())));
        for entry in &record.entries {
            rows.extend(result(entry));
        }
    }
    rows
}

fn route(app: &App, job: &Job) -> Vec<Row> {
    let elapsed = job
        .finished
        .unwrap_or(app.now)
        .duration_since(job.started)
        .unwrap_or_default()
        .as_secs();
    let mut rows = vec![Row::plain(format!(
        "Route: local → local; elapsed ~{elapsed}s"
    ))];
    if let Some(id) = job.retry_of {
        rows.push(Row::plain(format!("Retry of job {id}")));
    }
    for source in &job.plan.operands {
        rows.push(Row::plain(format!("Source: {}", source.path.display())));
    }
    if let Some(destination) = &job.plan.destination {
        rows.push(Row::plain(format!(
            "Destination: {}",
            destination.display()
        )));
    }
    rows
}

fn disabled(job: &Job) -> Vec<Row> {
    if job.active() {
        return vec![Row::styled(
            "Retry and reconciliation wait for worker acknowledgment.",
            Role::Disabled,
        )];
    }
    if job.state == JobState::Critical {
        return vec![Row::styled(
            "Critical safety incident: retry and reconciliation disabled; acknowledgment cannot restore data.",
            Role::Error,
        )];
    }
    if job.state == JobState::Unknown && !job.can_reconcile() {
        return vec![Row::styled(
            "Reconciliation disabled: no retained ownership evidence. Retry and safe exit remain blocked.",
            Role::Warning,
        )];
    }
    if !job.can_retry() && !job.can_reconcile() {
        return vec![Row::styled(
            "Retry unavailable: no incomplete operands remain.",
            Role::Disabled,
        )];
    }
    Vec::new()
}

fn result(entry: &crate::operations::EntryResult) -> Vec<Row> {
    let role = match entry.outcome {
        Outcome::SafetyIncident | Outcome::Failed => Role::Error,
        Outcome::Changed | Outcome::OutcomeUnknown | Outcome::CopiedSourceRetained => Role::Warning,
        Outcome::Completed => Role::Status,
        _ => Role::Value,
    };
    let mut rows = vec![Row::styled(
        format!("{}: {}", outcome(entry.outcome), entry.source.display()),
        role,
    )];
    if let Some(destination) = &entry.destination {
        rows.push(Row::plain(format!(
            "  Destination: {}",
            destination.display()
        )));
    }
    rows.push(Row::plain(format!("  {}", entry.detail)));
    if entry.checksum_verified {
        rows.push(Row::plain("  SHA-256 verified"));
    }
    if let Some(path) = entry
        .recovery
        .as_ref()
        .and_then(|evidence| evidence.staging_path())
    {
        rows.push(Row::styled(
            format!("  Owned staging checkpoint: {}", path.display()),
            Role::Warning,
        ));
    }
    if let Some(trash) = &entry.trash {
        rows.push(Row::plain(format!("  {}", trash.receipt.description())));
        rows.push(Row::plain(format!("  Validated inventory: {} files, {} directories (not individual Trash acknowledgments)", trash.validated_files, trash.validated_directories)));
        if let crate::operations::trash::Receipt::Linux(receipt) = &trash.receipt {
            rows.push(Row::plain(format!(
                "  Recovery metadata: {}",
                receipt.info.display()
            )));
        }
    }
    rows
}

fn state(state: JobState) -> &'static str {
    match state {
        JobState::Queued => "Queued",
        JobState::Running => "Running",
        JobState::Completed => "Completed",
        JobState::Skipped => "Skipped",
        JobState::Failed => "Failed",
        JobState::Critical => "CRITICAL safety incident",
        JobState::Partial => "Partial result",
        JobState::Cancelled => "Cancelled",
        JobState::Unknown => "Outcome unknown",
    }
}

fn state_role(state: JobState) -> Role {
    match state {
        JobState::Critical | JobState::Failed => Role::Error,
        JobState::Partial | JobState::Unknown => Role::Warning,
        JobState::Completed => Role::Status,
        _ => Role::Value,
    }
}

fn outcome(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Completed => "Completed",
        Outcome::Skipped => "Skipped",
        Outcome::Failed => "Failed",
        Outcome::Changed => "Changed since approval",
        Outcome::Cancelled => "Cancelled",
        Outcome::CopiedSourceRetained => "Copied; source retained",
        Outcome::OutcomeUnknown => "Outcome unknown",
        Outcome::SafetyIncident => "CRITICAL: OS reported permanent deletion during Trash",
    }
}

fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Reading => "Reading",
        Stage::Verified => "Verified",
        Stage::Committed => "Committed",
        Stage::Removing => "Removing source",
    }
}
