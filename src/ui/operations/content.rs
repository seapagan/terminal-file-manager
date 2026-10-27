use crate::{
    app::App,
    color::Role,
    fs::{Entry, EntryKind},
    operations::{Dialog, Operation, OperationDialog},
};

pub(super) type Row = crate::ui::modal::Row<usize>;

pub(super) fn title(app: &App) -> &'static str {
    match &app.operations.dialog {
        Some(Dialog::IncidentExit { .. }) => "Critical incident — acknowledge exit",
        Some(Dialog::Exit { .. }) => "Active jobs — safe exit",
        Some(Dialog::Operation(dialog)) => operation(dialog.plan.operation),
        None if app.operations.show_history => "Jobs / progress / results",
        None if !app.operations.conflicts.is_empty() => super::conflict::title(app),
        _ => "Waiting for acknowledged completion",
    }
}

pub(super) fn title_role(app: &App) -> Role {
    if matches!(&app.operations.dialog, Some(Dialog::Operation(d)) if d.plan.operation == Operation::Delete)
    {
        Role::Error
    } else {
        Role::Header
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

pub(super) fn wrapped(rows: Vec<Row>, width: u16) -> Vec<Row> {
    rows.into_iter().flat_map(|row| row.wrap(width)).collect()
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
            |_| Vec::new(),
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
        Row::field("Acknowledgment", typed, 0, true),
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
    sources(&dialog.plan.operands)
        .into_iter()
        .filter(|row| row.trailing.is_some())
        .collect()
}

pub(super) fn header(app: &App) -> Vec<Row> {
    let Some(Dialog::Operation(dialog)) = &app.operations.dialog else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    if dialog.plan.operation != Operation::Mkdir {
        rows.extend(
            if matches!(dialog.plan.operation, Operation::Delete | Operation::Trash) {
                destructive(dialog, app.operations.preview_frame)
            } else {
                vec![Row::styled(
                    selected_summary(&dialog.plan.operands),
                    Role::Status,
                )]
            },
        );
        if let Some(root) =
            crate::ui::modal::common_root(dialog.plan.operands.iter().map(|e| e.path.as_path()))
        {
            rows.push(Row::styled(
                format!("From: {}", root.display()),
                Role::Label,
            ));
        }
    }
    if !matches!(dialog.plan.operation, Operation::Delete | Operation::Trash) {
        rows.extend(form(dialog));
    }
    rows
}

fn form(dialog: &OperationDialog) -> Vec<Row> {
    let mut rows = Vec::new();
    if dialog.plan.operation == Operation::Mkdir {
        rows.push(Row::styled(
            format!("Parent: {}", dialog.base.display()),
            Role::Muted,
        ));
        rows.push(Row::plain(""));
    }
    rows.extend(rename_notice(dialog));
    rows.push(primary_field(dialog));
    if matches!(dialog.plan.operation, Operation::Copy | Operation::Move) {
        rows.extend(transfer(dialog));
    }
    if matches!(
        dialog.plan.operation,
        Operation::Permissions(_) | Operation::ReadOnly(_)
    ) {
        rows.push(Row::plain(
            "Only selected entries change; links are not followed.",
        ));
    }
    rows
}

fn native_destination(dialog: &OperationDialog) -> bool {
    matches!(dialog.plan.operation, Operation::Copy | Operation::Move)
        && !dialog.edited
        && dialog.native_target.to_str().is_none()
}

fn primary_field(dialog: &OperationDialog) -> Row {
    let value = if native_destination(dialog) {
        std::borrow::Cow::Owned(format!("{:?}", dialog.native_target))
    } else {
        std::borrow::Cow::Borrowed(dialog.text.as_str())
    };
    let field = Row::field(
        field_label(dialog.plan.operation),
        &value,
        0,
        dialog.field == 0,
    );
    if matches!(dialog.plan.operation, Operation::Copy | Operation::Move) {
        field.label_width(6)
    } else {
        field
    }
}

pub(super) fn footer(app: &App) -> Vec<Row> {
    let Some(Dialog::Operation(dialog)) = &app.operations.dialog else {
        return Vec::new();
    };
    dialog_error(dialog)
}

fn selected_summary(entries: &[Entry]) -> String {
    let directories = entries
        .iter()
        .filter(|e| e.kind == EntryKind::Directory)
        .count() as u64;
    format!(
        "{} selected",
        counts(entries.len() as u64 - directories, directories)
    )
}

fn counts(files: u64, directories: u64) -> String {
    let mut parts = Vec::new();
    if files > 0 {
        parts.push(format!(
            "{files} {}",
            if files == 1 { "file" } else { "files" }
        ));
    }
    if directories > 0 {
        parts.push(format!(
            "{directories} {}",
            if directories == 1 {
                "directory"
            } else {
                "directories"
            }
        ));
    }
    if parts.is_empty() {
        parts.push("0 entries".into());
    }
    parts.join(", ")
}

fn rename_notice(dialog: &OperationDialog) -> Vec<Row> {
    let mut rows = Vec::new();
    if dialog.plan.operation == Operation::Rename && dialog.plan.operands[0].name.to_str().is_none()
    {
        rows.push(Row::styled(
            "Original name is escaped for display. Ctrl+A and type a replacement; Esc cancels.",
            Role::Warning,
        ));
    }
    rows
}

pub(super) const PREVIEW_ERROR: &str =
    "Preview unavailable — confirmation disabled. Cancel and review the selected paths.";

fn dialog_error(dialog: &OperationDialog) -> Vec<Row> {
    let Some(error) = &dialog.error else {
        return Vec::new();
    };
    let primary = if matches!(dialog.plan.operation, Operation::Delete | Operation::Trash) {
        PREVIEW_ERROR
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
        _ => "To",
    }
}

fn transfer(dialog: &OperationDialog) -> Vec<Row> {
    let mut rows = Vec::new();
    if native_destination(dialog) {
        rows.push(Row::styled(
            "Native destination is display only; typing replaces it.",
            Role::Label,
        ));
    }
    if dialog.plan.operands.len() == 1 {
        rows.push(Row::field(
            "Incoming name (optional)",
            &dialog.incoming,
            1,
            dialog.field == 1,
        ));
        rows.push(Row::styled(
            "Tab switches fields; blank incoming name preserves the original.",
            Role::Label,
        ));
    }
    rows.push(Row::plain(if dialog.options.checksum {
        "Verification: SHA-256 + metadata"
    } else {
        "Verification: Metadata"
    }));
    rows
}

fn destructive(dialog: &OperationDialog, frame: u8) -> Vec<Row> {
    if dialog.error.is_some() {
        return vec![
            Row::styled("Preview failed — confirmation disabled", Role::Error).single_line(),
        ];
    }
    vec![
        Row::styled(
            dialog.plan.preview.as_ref().map_or_else(
                || {
                    format!(
                        "{} Counting recursively… confirmation disabled",
                        ['|', '/', '-', '\\'][usize::from(frame % 4)]
                    )
                },
                |preview| {
                    format!(
                        "{} · {}",
                        counts(preview.files, preview.directories),
                        crate::ui::modal::file_size(Some(preview.bytes))
                    )
                },
            ),
            if dialog.plan.preview.is_some() {
                Role::Status
            } else {
                Role::Disabled
            },
        )
        .single_line(),
    ]
}

/// Display-only lexical grouping; approved operands and symlink spelling stay untouched.
pub(super) fn sources(entries: &[Entry]) -> Vec<Row> {
    let root = crate::ui::modal::common_root(entries.iter().map(|entry| entry.path.as_path()));
    let mut rows = Vec::new();
    if let Some(root) = &root {
        rows.push(Row::styled(
            format!("From: {}", root.display()),
            Role::Label,
        ));
    }
    for entry in entries {
        let path = root
            .as_ref()
            .and_then(|root| entry.path.strip_prefix(root).ok())
            .unwrap_or(&entry.path);
        rows.push(crate::ui::modal::entry_row(entry, path));
    }
    rows
}
