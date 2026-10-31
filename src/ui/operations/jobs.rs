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
            label: String::new(),
            enabled: true,
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
                    phase(job)
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
        "Up/Down selects · PageUp/PageDown scroll details · evidence is session only",
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
            phase(job)
        ),
        state_role(job.state),
    ));
    if job.active() && job.record.is_some() {
        rows.push(Row::styled(
            "Previous result — retained during reconciliation",
            Role::Warning,
        ));
    }
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
        rows.extend(results(record, &job.plan.operands));
    }
    rows
}

fn results(record: &crate::operations::Record, operands: &[crate::fs::Entry]) -> Vec<Row> {
    let mut rows = Vec::new();
    rows.push(Row::plain(format!("Observed bytes: {}", record.bytes())));
    let root =
        crate::ui::modal::common_root(record.entries.iter().map(|entry| entry.source.as_path()));
    if let Some(root) = &root {
        rows.push(Row::styled(
            format!("Results from: {}", root.display()),
            Role::Label,
        ));
    }
    let operands: std::collections::HashMap<_, _> = operands
        .iter()
        .map(|entry| (entry.path.as_path(), entry))
        .collect();
    for entry in &record.entries {
        rows.extend(result(
            entry,
            root.as_deref(),
            operands.get(entry.source.as_path()).copied(),
        ));
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
    rows.extend(content::sources(&job.plan.operands));
    for operand in &job.plan.operands {
        rows.push(Row::plain(format!(
            "Planned source: {}",
            native_path(&operand.path)
        )));
    }
    if let Some(destination) = &job.plan.destination {
        rows.push(Row::plain(format!(
            "Destination: {}",
            native_path(destination)
        )));
    }
    rows
}

fn disabled(job: &Job) -> Vec<Row> {
    if job.active() {
        return vec![Row::styled(
            "Retry and reconciliation wait for worker acknowledgment.",
            Role::Value,
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
            Role::Value,
        )];
    }
    Vec::new()
}

fn result(
    entry: &crate::operations::EntryResult,
    root: Option<&std::path::Path>,
    operand: Option<&crate::fs::Entry>,
) -> Vec<Row> {
    let role = match entry.outcome {
        Outcome::SafetyIncident | Outcome::Failed => Role::Error,
        Outcome::Changed | Outcome::OutcomeUnknown | Outcome::CopiedSourceRetained => Role::Warning,
        Outcome::Completed => Role::Status,
        _ => Role::Value,
    };
    let source = root
        .and_then(|root| entry.source.strip_prefix(root).ok())
        .unwrap_or(&entry.source);
    let mut heading = result_heading(entry, source, operand);
    heading.role = role;
    let mut rows = vec![heading, Row::styled(outcome(entry.outcome), role)];
    rows.push(Row::plain(format!(
        "  Source: {}",
        native_path(&entry.source)
    )));
    if let Some(destination) = &entry.destination {
        rows.push(Row::plain(format!(
            "  Destination: {}",
            native_path(destination)
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
            format!("  Owned staging checkpoint: {}", native_path(path)),
            Role::Warning,
        ));
    }
    if let Some(trash) = &entry.trash {
        rows.extend(trash_evidence(trash));
    }
    rows
}

fn result_heading(
    entry: &crate::operations::EntryResult,
    source: &std::path::Path,
    operand: Option<&crate::fs::Entry>,
) -> Row {
    let mut heading = operand.map_or_else(
        || {
            Row::file(
                format!(
                    "{}  {}{}",
                    super::super::listing::type_indicator(entry.kind, false, false),
                    source.display(),
                    if entry.kind == crate::fs::EntryKind::Directory {
                        "/"
                    } else {
                        ""
                    }
                ),
                if entry.kind == crate::fs::EntryKind::File {
                    crate::ui::modal::file_size(Some(entry.bytes))
                } else {
                    "—".into()
                },
            )
        },
        |operand| crate::ui::modal::entry_row(operand, source),
    );
    if entry.kind == crate::fs::EntryKind::File {
        heading.trailing = Some(crate::ui::modal::file_size(Some(entry.bytes)));
    }
    heading
}

pub(super) fn phase(job: &Job) -> String {
    if job.active() && job.record.is_some() {
        format!("Reconciliation · {}", state(job.state))
    } else {
        state(job.state).into()
    }
}

pub(super) fn state(state: JobState) -> &'static str {
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

pub(super) fn state_role(state: JobState) -> Role {
    match state {
        JobState::Critical | JobState::Failed => Role::Error,
        JobState::Partial | JobState::Unknown => Role::Warning,
        JobState::Completed => Role::Status,
        _ => Role::Value,
    }
}

pub(super) fn outcome(outcome: Outcome) -> &'static str {
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

pub(super) fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Reading => "Reading",
        Stage::Verified => "Verified",
        Stage::Committed => "Committed",
        Stage::Removing => "Removing source",
    }
}

fn native_path(path: &std::path::Path) -> String {
    path.to_str().map_or_else(
        || format!("{:?} (escaped native path)", path.as_os_str()),
        str::to_owned,
    )
}

fn trash_evidence(trash: &crate::operations::TrashResult) -> Vec<Row> {
    let mut rows = Vec::new();
    rows.push(Row::plain(format!("  {}", trash.receipt.description())));
    rows.push(Row::plain(format!(
        "  Validated inventory: {} files, {} directories (not individual Trash acknowledgments)",
        trash.validated_files, trash.validated_directories
    )));
    use crate::operations::trash::Receipt;
    match &trash.receipt {
        Receipt::Linux(receipt) => {
            rows.push(Row::plain(format!(
                "  Native Trash target: {}",
                native_path(&receipt.path)
            )));
            rows.push(Row::plain(format!(
                "  Recovery metadata: {}",
                native_path(&receipt.info)
            )));
        }
        Receipt::Mac { path: Some(path) } => {
            rows.push(Row::plain(format!(
                "  Native Trash target: {}",
                native_path(path)
            )));
        }
        Receipt::Windows {
            parsing_name,
            metadata,
        } => {
            rows.extend(windows_receipt(parsing_name.as_deref(), metadata));
        }
        Receipt::Mac { path: None } => {}
    }
    rows
}

fn windows_receipt(
    parsing_name: Option<&[u16]>,
    metadata: &crate::operations::trash::ShellMetadata,
) -> Vec<Row> {
    let mut rows = Vec::new();
    for (label, value) in [
        ("Shell parsing name", parsing_name),
        ("Display name", metadata.display_name.as_deref()),
        ("Deleted from", metadata.deleted_from.as_deref()),
    ] {
        if let Some(value) = value {
            rows.push(Row::plain(format!(
                "  {label}: {}",
                String::from_utf16_lossy(value)
            )));
            rows.push(Row::plain(format!("  {label} (exact UTF-16): {value:?}")));
        }
    }
    if let Some(time) = metadata.deletion_time {
        rows.push(Row::plain(format!(
            "  Deletion time (FILETIME ticks): {time}"
        )));
    }
    rows
}
