use crate::{
    app::App,
    color::{Role, Theme},
    operations::{Dialog, Event, Operation},
};
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

#[derive(Clone, Debug)]
pub struct OperationControl {
    pub area: Rect,
    pub key: KeyCode,
    pub label: &'static str,
}

fn area(terminal: Rect) -> Rect {
    super::centered(terminal, 100, 26)
}

fn visible(app: &App) -> bool {
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

pub(super) fn controls(app: &App, terminal: Rect) -> Vec<OperationControl> {
    if !visible(app) {
        return Vec::new();
    }
    let rectangle = area(terminal);
    let mut controls = Vec::new();
    let mut x = rectangle.x.saturating_add(2);
    for (label, key) in buttons(app) {
        let width = label.len() as u16 + 2;
        if x.saturating_add(width) > rectangle.right().saturating_sub(1) {
            break;
        }
        controls.push(OperationControl {
            area: Rect::new(x, rectangle.bottom().saturating_sub(2), width, 1),
            key,
            label,
        });
        x = x.saturating_add(width + 1);
    }
    controls
}

fn buttons(app: &App) -> Vec<(&'static str, KeyCode)> {
    match &app.operations.dialog {
        Some(Dialog::Exit { .. }) => vec![
            ("R Return", KeyCode::Char('r')),
            ("W Wait then exit", KeyCode::Char('w')),
            ("C Cancel safely then exit", KeyCode::Char('c')),
        ],
        Some(Dialog::Operation(dialog)) => match dialog.plan.operation {
            Operation::Delete if dialog.ready => vec![
                ("Enter Cancel", KeyCode::Enter),
                ("D DELETE permanently", KeyCode::Char('D')),
            ],
            Operation::Trash if dialog.ready => vec![
                ("Enter Cancel", KeyCode::Enter),
                ("T Trash", KeyCode::Char('T')),
            ],
            Operation::Delete | Operation::Trash => vec![("Enter Cancel", KeyCode::Enter)],
            _ => vec![
                ("Enter Run", KeyCode::Enter),
                ("Esc Cancel", KeyCode::Esc),
                ("F2 SHA-256", KeyCode::F(2)),
            ],
        },
        None if app.operations.show_history => vec![
            ("X Reconcile", KeyCode::Char('x')),
            ("R Retry incomplete", KeyCode::Char('r')),
            ("C Cancel job", KeyCode::Char('c')),
            ("Esc Close", KeyCode::Esc),
        ],
        None if !app.operations.conflicts.is_empty() => vec![
            ("S Skip", KeyCode::Char('s')),
            ("R Replace", KeyCode::Char('r')),
            ("K Keep both", KeyCode::Char('k')),
            ("N Newer", KeyCode::Char('n')),
            ("A All", KeyCode::Char('a')),
            ("Esc Cancel job", KeyCode::Esc),
        ],
        _ => vec![("Esc Return to manager", KeyCode::Esc)],
    }
}

pub(super) fn scroll_limit(app: &App, terminal: Rect) -> u16 {
    let rectangle = area(terminal);
    wrapped(app, rectangle.width.saturating_sub(4))
        .len()
        .saturating_sub(usize::from(rectangle.height.saturating_sub(5)))
        .min(u16::MAX as usize) as u16
}

pub(super) fn render(frame: &mut Frame, app: &App, theme: &Theme, controls: &[OperationControl]) {
    notices(frame, app, theme);
    if !visible(app) {
        return;
    }
    let rectangle = area(frame.area());
    frame.render_widget(Clear, rectangle);
    frame.render_widget(
        Block::default()
            .title(title(app))
            .borders(Borders::ALL)
            .border_style(theme.style(Role::ActivePane))
            .style(theme.style(Role::Normal)),
        rectangle,
    );
    let body = Rect::new(
        rectangle.x.saturating_add(2),
        rectangle.y.saturating_add(1),
        rectangle.width.saturating_sub(4),
        rectangle.height.saturating_sub(4),
    );
    let scroll = app
        .operations
        .scroll
        .min(usize::from(scroll_limit(app, frame.area())));
    let text: Vec<_> = wrapped(app, body.width)
        .into_iter()
        .map(Line::raw)
        .collect();
    frame.render_widget(
        Paragraph::new(text)
            .style(theme.style(Role::Value))
            .scroll((scroll as u16, 0)),
        body,
    );
    for control in controls {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("[{}]", control.label),
                theme.style(Role::Key),
            )),
            control.area,
        );
    }
}

fn title(app: &App) -> &'static str {
    match &app.operations.dialog {
        Some(Dialog::Exit { .. }) => "Active jobs — safe exit",
        Some(Dialog::Operation(dialog)) if dialog.plan.operation == Operation::Delete => {
            "PERMANENT DELETE — irreversible"
        }
        Some(Dialog::Operation(_)) => "Local operation",
        None if app.operations.show_history => "Jobs / progress / results",
        None if !app.operations.conflicts.is_empty() => "Destination conflict — Skip is default",
        _ => "Waiting for acknowledged completion — Esc returns",
    }
}

fn wrapped(app: &App, width: u16) -> Vec<String> {
    lines(app)
        .into_iter()
        .flat_map(|text| super::text::wrap_value(&super::clean(&text), usize::from(width)))
        .collect()
}

fn lines(app: &App) -> Vec<String> {
    if let Some(Dialog::Exit { save }) = app.operations.dialog {
        return vec![
            format!(
                "Exit intent: {}",
                if save {
                    "save navigation session (Q)"
                } else {
                    "ordinary quit"
                }
            ),
            "Default: Return to manager. No jobs detach.".into(),
            "Wait then exit: keep jobs running until acknowledged.".into(),
            "Cancel safely then exit: stop submissions, await workers and cleanup.".into(),
            "An unknown outcome blocks safe exit and retry.".into(),
        ];
    }
    if let Some(Dialog::Operation(dialog)) = &app.operations.dialog {
        return dialog_lines(dialog);
    }
    if app.operations.show_history {
        return history(app);
    }
    if let Some(conflict) = app.operations.conflicts.front() {
        return conflict_lines(conflict);
    }
    vec!["Jobs remain attached to this manager. Esc abandons the exit request.".into()]
}

fn dialog_lines(dialog: &crate::operations::OperationDialog) -> Vec<String> {
    let mut lines = vec![
        format!("Operation: {:?} — Local only", dialog.plan.operation),
        format!("{} approved root operand(s)", dialog.plan.operands.len()),
    ];
    if matches!(dialog.plan.operation, Operation::Delete | Operation::Trash) {
        lines.push(if dialog.plan.operation == Operation::Delete {
            "PERMANENT DELETE cannot be undone. Default: Cancel.".into()
        } else {
            "Native Trash only. No permanent-delete fallback. Default: Cancel.".into()
        });
        lines.push(dialog.plan.preview.as_ref().map_or_else(
            || "Counting recursively… confirmation disabled".into(),
            |preview| {
                format!(
                    "Preview: {} files, {} directories, {} bytes",
                    preview.files, preview.directories, preview.bytes
                )
            },
        ));
    } else {
        lines.push(format!(
            "{} Path/name/mode: {}",
            if dialog.field == 0 { ">" } else { " " },
            dialog.text
        ));
        if matches!(dialog.plan.operation, Operation::Copy | Operation::Move) {
            lines.extend(transfer_lines(dialog));
        }
    }
    lines.push("PgUp/PgDown scroll; Esc cancels".into());
    for source in &dialog.plan.operands {
        lines.push(format!(
            "{} [{:?}, {} bytes]",
            source.path.display(),
            source.kind,
            source.size.unwrap_or(0)
        ));
    }
    lines
}

fn transfer_lines(dialog: &crate::operations::OperationDialog) -> Vec<String> {
    let mut lines = vec![format!("Native target: {}", dialog.native_target.display())];
    if dialog.plan.operands.len() == 1 {
        lines.push(format!(
            "{} Optional incoming name: {} (Tab switches fields)",
            if dialog.field == 1 { ">" } else { " " },
            dialog.incoming
        ));
    }
    lines.push(format!(
        "Verification: {}; F2 toggles SHA-256",
        if dialog.options.checksum {
            "SHA-256 + metadata"
        } else {
            "metadata (not byte equality)"
        }
    ));
    lines
}

fn conflict_lines(conflict: &crate::operations::PendingConflict) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Job {} — arrows select; Enter: {}",
            conflict.id,
            [
                "Skip (default)",
                "Replace",
                "Keep both",
                "Newer",
                "Cancel job"
            ][conflict.choice.min(4)]
        ),
        format!(
            "Apply to all in this job and type pair: {} (A toggles)",
            conflict.all
        ),
    ];
    if let Event::Conflict {
        source,
        destination,
        newer_available,
    } = &conflict.event
    {
        for (label, entry) in [("Source", source), ("Destination", destination)] {
            lines.extend(identity_lines(label, entry));
        }
        lines.push(format!(
            "Newer-only: {}",
            if *newer_available {
                "available"
            } else {
                "disabled: timestamps/types untrustworthy"
            }
        ));
        if source.may_recurse() != destination.may_recurse() {
            lines.push(
                "Replace disabled: no safe atomic type replacement. Keep both or Skip.".into(),
            );
        }
    }
    lines.push("S Skip; R Replace; K Keep both; N Newer; A All; Esc Cancel job; F9 Jobs".into());
    lines
}

fn identity_lines(label: &str, entry: &crate::fs::Entry) -> Vec<String> {
    vec![
        format!("{label}: {}", entry.path.display()),
        format!(
            "  {:?}; size {:?}; modified {:?}; identity {:?}",
            entry.kind,
            entry.size,
            entry.modified,
            entry.identity.as_ref().and_then(|id| id.object)
        ),
    ]
}

fn history(app: &App) -> Vec<String> {
    let mut lines = vec!["Up/Down selects; Enter resolves a pending conflict; R retries incomplete; C cancels; X reconciles unknown outcomes without repeating source removal".into()];
    for (index, job) in app.operations.history.iter().enumerate() {
        lines.push(format!(
            "{} Job {} {:?} {:?}",
            if index == app.operations.cursor {
                ">"
            } else {
                " "
            },
            job.id,
            job.plan.operation,
            job.state
        ));
    }
    if let Some(job) = app.operations.history.get(app.operations.cursor) {
        lines.extend(job_lines(app, job));
    }
    lines
}

fn job_lines(app: &App, job: &crate::operations::Job) -> Vec<String> {
    let mut lines = vec![crate::operations::summary(job)];
    lines.push(format!(
        "Route: Local → local; retry of {:?}; elapsed ~{}s",
        job.retry_of,
        job.finished
            .unwrap_or(app.now)
            .duration_since(job.started)
            .unwrap_or_default()
            .as_secs()
    ));
    if let Some(destination) = &job.plan.destination {
        lines.push(format!("Destination: {}", destination.display()));
    }
    if let Some(Event::Progress {
        source,
        stage,
        bytes,
        ..
    }) = &job.progress
    {
        lines.push(format!(
            "{stage:?}: {} — {bytes} current-file bytes (sampled); rate/ETA unavailable",
            source.display()
        ));
    }
    if let Some(record) = &job.record {
        for entry in &record.entries {
            lines.push(format!(
                "{:?}: {} → {:?}: {}",
                entry.outcome,
                entry.source.display(),
                entry.destination,
                entry.detail
            ));
            if let Some(path) = entry
                .recovery
                .as_ref()
                .and_then(|evidence| evidence.staging_path())
            {
                lines.push(format!("Owned staging checkpoint: {}", path.display()));
            }
        }
    }
    lines
}

fn notices(frame: &mut Frame, app: &App, theme: &Theme) {
    if app.diagnostics || app.help || app.popup || app.operations.dialog.is_some() {
        return;
    }
    let notices: Vec<_> = app
        .operations
        .notices
        .iter()
        .filter(|notice| notice.expires > app.now)
        .collect();
    let bottom = frame.area().bottom().saturating_sub(2);
    for (index, notice) in notices.iter().enumerate() {
        let area = Rect::new(
            frame.area().x,
            bottom.saturating_sub((notices.len() - index) as u16),
            frame.area().width,
            1,
        );
        let role = if notice.warning {
            Role::Warning
        } else {
            Role::Status
        };
        frame.render_widget(
            Paragraph::new(format!("[job] {}", super::clean(&notice.text)))
                .style(theme.style(role)),
            area,
        );
    }
}
