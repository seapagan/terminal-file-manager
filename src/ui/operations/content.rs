use crate::{
    app::App,
    color::Role,
    fs::{Entry, EntryKind},
    operations::{Dialog, Event, Operation, OperationDialog},
};

pub(super) struct Row {
    pub text: String,
    pub role: Role,
    pub field: Option<usize>,
}
impl Row {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            role: Role::Value,
            field: None,
        }
    }
    pub fn styled(text: impl Into<String>, role: Role) -> Self {
        Self {
            text: text.into(),
            role,
            field: None,
        }
    }
    fn field(label: &str, value: &str, index: usize, selected: usize) -> Self {
        Self {
            text: format!(
                "{}{label}: {value}",
                if index == selected { "> " } else { "  " }
            ),
            role: if index == selected {
                Role::Cursor
            } else {
                Role::Value
            },
            field: Some(index),
        }
    }
}

pub(super) fn title(app: &App) -> &'static str {
    match &app.operations.dialog {
        Some(Dialog::IncidentExit { .. }) => "Critical incident — acknowledge exit",
        Some(Dialog::Exit { .. }) => "Active jobs — safe exit",
        Some(Dialog::Operation(dialog)) => operation(dialog.plan.operation),
        None if app.operations.show_history => "Jobs / progress / results",
        None if !app.operations.conflicts.is_empty() => "Destination conflict — Skip is default",
        _ => "Waiting for acknowledged completion",
    }
}

pub(super) fn operation(operation: Operation) -> &'static str {
    match operation {
        Operation::Copy => "Copy",
        Operation::Move => "Move",
        Operation::Rename => "Rename",
        Operation::Mkdir => "Create directory",
        Operation::Permissions(_) => "Change permissions",
        Operation::ReadOnly(_) => "Change read-only attribute",
        Operation::Trash => "Move to Trash",
        Operation::Delete => "PERMANENT DELETE — irreversible",
    }
}

pub(super) fn wrapped(app: &App, width: u16) -> Vec<Row> {
    rows(app)
        .into_iter()
        .flat_map(|row| {
            super::super::text::wrap_value(&super::super::clean(&row.text), usize::from(width))
                .into_iter()
                .map(move |text| Row {
                    text,
                    role: row.role,
                    field: row.field,
                })
        })
        .collect()
}

pub(super) fn rows(app: &App) -> Vec<Row> {
    match &app.operations.dialog {
        Some(Dialog::IncidentExit { typed, .. }) => incident(app, typed),
        Some(Dialog::Exit { save }) => exit(*save),
        Some(Dialog::Operation(dialog)) => dialog_rows(dialog),
        None if app.operations.show_history => super::jobs::details(app),
        None => app.operations.conflicts.front().map_or_else(
            || {
                vec![Row::plain(
                    "Jobs remain attached. Esc returns to the manager.",
                )]
            },
            conflict,
        ),
    }
}

fn incident(app: &App, typed: &str) -> Vec<Row> {
    let mut rows = vec![
        Row::styled(
            "CRITICAL: the OS reported permanent deletion during F8 Trash.",
            Role::Error,
        ),
        Row::plain("This is irreversible. Acknowledgment does not restore deleted data."),
        Row::plain(
            "Jobs evidence is session only and is lost on exit. Record the F9 details first.",
        ),
        Row::plain("Enter or Esc: Stay. Type ACKNOWLEDGE manually, then press F10 to exit."),
        Row::field("Acknowledgment", typed, 0, 0),
    ];
    if app.operations.active() {
        rows.push(Row::styled(
            "Exit disabled: active workers must finish or acknowledge cancellation.",
            Role::Warning,
        ));
    }
    if app.operations.unknown() {
        rows.push(Row::styled(
            "Exit disabled: an ordinary unknown outcome needs reconciliation.",
            Role::Warning,
        ));
    }
    rows
}

fn exit(save: bool) -> Vec<Row> {
    vec![
        Row::plain(if save {
            "Exit and save the navigation session (Q)."
        } else {
            "Exit without saving the navigation session."
        }),
        Row::plain("Default: Return to manager. No jobs detach."),
        Row::plain("Wait: finish active jobs before exit."),
        Row::plain("Cancel safely: stop submissions and wait for worker cleanup."),
        Row::styled(
            "Unknown outcomes block safe exit until reconciled.",
            Role::Warning,
        ),
    ]
}

fn dialog_rows(dialog: &OperationDialog) -> Vec<Row> {
    let mut rows = dialog_error(dialog);
    if dialog.plan.operation != Operation::Mkdir {
        rows.push(Row::plain(format!(
            "Local operation: {} selected {}",
            dialog.plan.operands.len(),
            if dialog.plan.operands.len() == 1 {
                "entry"
            } else {
                "entries"
            }
        )));
    }
    if matches!(dialog.plan.operation, Operation::Delete | Operation::Trash) {
        rows.extend(destructive(dialog));
    } else {
        rows.push(Row::field(
            field_label(dialog.plan.operation),
            &dialog.text,
            0,
            dialog.field,
        ));
        if matches!(dialog.plan.operation, Operation::Copy | Operation::Move) {
            rows.extend(transfer(dialog));
        }
        if dialog.plan.operation == Operation::Mkdir {
            rows.push(Row::plain(format!("Parent: {}", dialog.base.display())));
        }
        if matches!(
            dialog.plan.operation,
            Operation::Permissions(_) | Operation::ReadOnly(_)
        ) {
            rows.push(Row::plain(
                "Only selected entries change; links are not followed.",
            ));
        }
    }
    for source in &dialog.plan.operands {
        rows.push(Row::plain(format!(
            "{} · {} · {}",
            source.path.display(),
            kind(source.kind),
            super::super::listing::size(source.size)
        )));
    }
    rows
}

fn dialog_error(dialog: &OperationDialog) -> Vec<Row> {
    let Some(error) = &dialog.error else {
        return Vec::new();
    };
    let primary = if matches!(dialog.plan.operation, Operation::Delete | Operation::Trash) {
        "Preview unavailable — confirmation disabled. Cancel and review the selected paths."
    } else {
        "Request not started — correct the fields or cancel."
    };
    vec![
        Row::styled(primary, Role::Error),
        Row::styled(error, Role::Error),
    ]
}

fn field_label(operation: Operation) -> &'static str {
    match operation {
        Operation::Mkdir => "Directory name",
        Operation::Rename => "New name",
        Operation::Permissions(_) => "Permissions (octal)",
        Operation::ReadOnly(_) => "Read-only (true/false)",
        _ => "Destination directory",
    }
}

fn transfer(dialog: &OperationDialog) -> Vec<Row> {
    let mut rows = Vec::new();
    if dialog.plan.operands.len() == 1 {
        rows.push(Row::field(
            "Incoming name (optional)",
            &dialog.incoming,
            1,
            dialog.field,
        ));
        rows.push(Row::styled(
            "Tab switches fields; blank incoming name preserves the original.",
            Role::Label,
        ));
    }
    if !dialog.edited {
        rows.push(Row::plain(format!(
            "Approved destination: {}",
            dialog.native_target.display()
        )));
    }
    rows.push(Row::plain(if dialog.options.checksum {
        "Verification: SHA-256 + metadata; F2 toggles"
    } else {
        "Verification: metadata (not byte equality); F2 toggles SHA-256"
    }));
    rows
}

fn destructive(dialog: &OperationDialog) -> Vec<Row> {
    let mut rows = vec![Row::styled(
        if dialog.plan.operation == Operation::Delete {
            "PERMANENT DELETE cannot be undone. Default: Cancel."
        } else {
            "Native Trash only; no permanent-delete fallback. Default: Cancel."
        },
        Role::Warning,
    )];
    if dialog.error.is_some() {
        return rows;
    }
    rows.push(Row::plain(dialog.plan.preview.as_ref().map_or_else(
        || "Counting recursively… confirmation disabled".into(),
        |preview| {
            format!(
                "Preview: {} {}, {} {}, {} bytes",
                preview.files,
                if preview.files == 1 { "file" } else { "files" },
                preview.directories,
                if preview.directories == 1 {
                    "directory"
                } else {
                    "directories"
                },
                preview.bytes
            )
        },
    )));
    rows
}

fn conflict(conflict: &crate::operations::PendingConflict) -> Vec<Row> {
    let choices = [
        "Skip (default)",
        "Replace",
        "Keep both",
        "Newer",
        "Cancel job",
    ];
    let mut rows = vec![
        Row::plain(format!(
            "Job {} — arrows select; Enter: {}",
            conflict.id,
            choices[conflict.choice.min(4)]
        )),
        Row::plain(format!(
            "Apply to all matching type pairs in this job: {} (A toggles)",
            if conflict.all { "yes" } else { "no" }
        )),
    ];
    if let Event::Conflict {
        source,
        destination,
        newer_available,
    } = &conflict.event
    {
        rows.extend(identity("Source", source));
        rows.extend(identity("Destination", destination));
        if source.may_recurse() != destination.may_recurse() {
            rows.push(Row::styled("Replace disabled: types differ; no safe atomic replacement. Use Keep both or Skip.", Role::Warning));
        }
        if !newer_available {
            rows.push(Row::styled(
                "Newer disabled: timestamps or types are untrustworthy.",
                Role::Disabled,
            ));
        }
    }
    rows
}

fn identity(label: &str, entry: &Entry) -> Vec<Row> {
    let identity = entry
        .identity
        .as_ref()
        .and_then(|id| id.object)
        .map_or_else(
            || "unavailable".into(),
            |(volume, object)| format!("volume {volume}, object {object}"),
        );
    vec![
        Row::styled(format!("{label}: {}", entry.path.display()), Role::Label),
        Row::plain(format!(
            "{}; size {}; modified {} UTC",
            kind(entry.kind),
            super::super::listing::size(entry.size),
            super::super::listing::timestamp(entry.modified)
        )),
        Row::plain(format!("Identity: {identity}")),
    ]
}

pub(super) fn kind(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Directory => "directory",
        EntryKind::File => "file",
        EntryKind::Link => "symbolic link",
        EntryKind::Junction => "junction",
        EntryKind::ReparsePoint => "reparse point",
        EntryKind::Socket => "socket",
        EntryKind::Fifo => "named pipe",
        EntryKind::Device => "device",
        EntryKind::Unknown => "unknown type",
    }
}
