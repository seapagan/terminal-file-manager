use super::Controller;
use crate::operations::{Dialog, Intent, Operation, OperationDialog, Plan};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::{
    io,
    path::{Component, PathBuf},
};

pub(super) fn handle(
    controller: &mut Controller,
    dialog: Dialog,
    event: &Event,
    geometry: &crate::ui::Geometry,
) -> Option<Dialog> {
    match dialog {
        Dialog::Exit { save } => super::operation_exit::choices(controller, save, event),
        Dialog::IncidentExit { save, typed } => {
            super::operation_exit::incident(controller, save, typed, event)
        }
        Dialog::Operation(mut dialog) => {
            let keep = match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    operation_key(controller, &mut dialog, *key, geometry)
                }
                Event::Paste(text) => {
                    append(controller, &mut dialog, text);
                    true
                }
                _ => true,
            };
            keep.then_some(Dialog::Operation(dialog))
        }
    }
}

fn operation_key(
    controller: &mut Controller,
    dialog: &mut OperationDialog,
    key: KeyEvent,
    geometry: &crate::ui::Geometry,
) -> bool {
    if matches!(key.code, KeyCode::Esc) {
        cancel_preview(controller, dialog);
        return false;
    }
    if matches!(dialog.plan.operation, Operation::Delete | Operation::Trash) {
        return destructive(controller, dialog, key.code, geometry);
    }
    match key.code {
        KeyCode::Enter => submit(controller, dialog, geometry),
        KeyCode::F(2) if matches!(dialog.plan.operation, Operation::Copy | Operation::Move) => {
            dialog.options.checksum = !dialog.options.checksum;
            true
        }
        KeyCode::Tab
            if dialog.plan.operands.len() == 1
                && matches!(dialog.plan.operation, Operation::Copy | Operation::Move) =>
        {
            dialog.field = 1 - dialog.field;
            true
        }
        KeyCode::Backspace => {
            field(dialog).pop();
            dialog.edited |= dialog.field == 0;
            true
        }
        KeyCode::Char('a') if key.modifiers == KeyModifiers::CONTROL => {
            field(dialog).clear();
            dialog.edited |= dialog.field == 0;
            true
        }
        KeyCode::Char(character)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            append(controller, dialog, &character.to_string());
            true
        }
        _ => true,
    }
}

fn destructive(
    controller: &mut Controller,
    dialog: &mut OperationDialog,
    key: KeyCode,
    geometry: &crate::ui::Geometry,
) -> bool {
    if key == KeyCode::Enter {
        cancel_preview(controller, dialog);
        return false;
    }
    let confirm = match dialog.plan.operation {
        Operation::Delete => key == KeyCode::Char('D'),
        _ => matches!(key, KeyCode::Char('t' | 'T')),
    };
    if !confirm {
        return true;
    }
    if !dialog.ready || dialog.plan.preview.is_none() || dialog.error.is_some() {
        controller.app.status = "Recursive preview pending or failed; confirmation disabled".into();
        return true;
    }
    submit(controller, dialog, geometry)
}

pub(super) fn cancel_preview(controller: &mut Controller, dialog: &OperationDialog) {
    if let Some(id) = dialog.preview_id {
        controller.app.operations.outbox.push(Intent::Cancel(id));
    }
    controller
        .app
        .operations
        .outbox
        .retain(|intent| !matches!(intent, Intent::Preview(_)));
    controller.app.operations.scroll = 0;
}

fn submit(
    controller: &mut Controller,
    dialog: &mut OperationDialog,
    geometry: &crate::ui::Geometry,
) -> bool {
    if dialog.plan.operation.creates_entry()
        && let Some(message) = dialog.name_error()
    {
        dialog.error = Some(message.into());
        controller.app.status = message.into();
        return true;
    }
    let approval = match dialog.plan.operation {
        Operation::Delete => Some(KeyCode::Char('D')),
        Operation::Trash => Some(KeyCode::Char('T')),
        Operation::Mkdir | Operation::CreateFile => Some(KeyCode::Enter),
        _ => None,
    };
    if (matches!(dialog.plan.operation, Operation::Delete | Operation::Trash)
        && (!dialog.ready || dialog.plan.preview.is_none() || dialog.error.is_some()))
        || approval.is_some_and(|key| !geometry.operation_enabled(key))
    {
        controller.app.status = "Approval disabled; check the form or enlarge the terminal".into();
        return true;
    }
    match prepare(controller, dialog) {
        Ok(true) => {
            let intent = dialog.retry_of.map_or_else(
                || Intent::Submit(dialog.plan.clone(), dialog.options),
                |id| Intent::SubmitRetry(dialog.plan.clone(), dialog.options, id),
            );
            controller.app.operations.outbox.push(intent);
            controller.app.operations.scroll = 0;
            false
        }
        Ok(false) => true,
        Err(error) => {
            dialog.error = Some(error.to_string());
            controller.app.status = "Operation not started; review the dialog details".into();
            true
        }
    }
}

fn prepare(controller: &mut Controller, dialog: &mut OperationDialog) -> io::Result<bool> {
    if dialog.track_scope && !dialog.plan.operation.creates_entry() {
        let fresh = Plan::capture(
            &controller.app.panes[dialog.pane],
            dialog.plan.operation,
            dialog.plan.destination.clone(),
        )?;
        if !same_scope(&dialog.plan, &fresh) {
            cancel_preview(controller, dialog);
            dialog.plan = fresh;
            if matches!(dialog.plan.operation, Operation::Delete | Operation::Trash) {
                dialog.ready = false;
                dialog.preview_id = None;
                controller
                    .app
                    .operations
                    .outbox
                    .push(Intent::Preview(dialog.plan.clone()));
            }
            controller.app.status =
                "Selection changed; review the displayed scope and approve again".into();
            return Ok(false);
        }
    }
    apply_fields(dialog)?;
    Ok(true)
}

fn same_scope(a: &Plan, b: &Plan) -> bool {
    a.operands.len() == b.operands.len()
        && a.operands.iter().all(|entry| {
            b.operands.iter().any(|other| {
                entry.path == other.path
                    && entry.identity == other.identity
                    && entry.kind == other.kind
            })
        })
}

fn apply_fields(dialog: &mut OperationDialog) -> io::Result<()> {
    match dialog.plan.operation {
        Operation::Copy | Operation::Move => {
            dialog.plan.set_destination(target(dialog)?);
            if !dialog.incoming.is_empty() {
                validate_name(&dialog.incoming)?;
                dialog.plan.incoming_name = Some(dialog.incoming.clone().into());
            }
        }
        Operation::Mkdir | Operation::CreateFile => {
            dialog.plan.originating_pane = Some(dialog.pane);
            dialog.plan.set_destination(dialog.base.join(&dialog.text));
        }
        Operation::Rename => {
            dialog.plan.set_destination(rename_target(dialog)?);
        }
        Operation::Permissions(_) => {
            let mode = u32::from_str_radix(dialog.text.trim().trim_start_matches("0o"), 8)
                .map_err(io::Error::other)?;
            if mode > 0o7777 {
                return Err(io::Error::other("Unix mode must be 0000..7777"));
            }
            dialog.plan.operation = Operation::Permissions(mode);
        }
        Operation::ReadOnly(_) => {
            dialog.plan.operation = Operation::ReadOnly(
                dialog
                    .text
                    .trim()
                    .parse::<bool>()
                    .map_err(io::Error::other)?,
            );
        }
        _ => {}
    }
    Ok(())
}

fn rename_target(dialog: &OperationDialog) -> io::Result<PathBuf> {
    if !dialog.edited && dialog.plan.operands[0].name.to_str().is_none() {
        return Err(io::Error::other(
            "Native name is display only; Ctrl+a and enter a replacement filename",
        ));
    }
    validate_name(&dialog.text)?;
    Ok(dialog.base.join(&dialog.text))
}

fn target(dialog: &OperationDialog) -> io::Result<PathBuf> {
    if dialog.edited {
        typed_path(dialog)
    } else {
        Ok(dialog.native_target.clone())
    }
}

fn typed_path(dialog: &OperationDialog) -> io::Result<PathBuf> {
    if dialog.text.is_empty() || dialog.text.contains('\0') {
        return Err(io::Error::other(
            "Enter an explicit nonempty path without NUL",
        ));
    }
    let path = PathBuf::from(&dialog.text);
    Ok(if path.is_absolute() {
        path
    } else {
        dialog.base.join(path)
    })
}

fn validate_name(text: &str) -> io::Result<()> {
    let path = std::path::Path::new(text);
    if text.contains('\0')
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        Err(io::Error::other(
            "Enter one native filename; use Move to change directories",
        ))
    } else {
        Ok(())
    }
}

fn field(dialog: &mut OperationDialog) -> &mut String {
    if dialog.field == 0 {
        &mut dialog.text
    } else {
        &mut dialog.incoming
    }
}

fn append(controller: &mut Controller, dialog: &mut OperationDialog, text: &str) {
    if field(dialog).len().saturating_add(text.len()) > 65_536 {
        controller.app.status = "Input too long; pasted input rejected".into();
        return;
    }
    dialog.error = None;
    field(dialog).push_str(text);
    dialog.edited |= dialog.field == 0;
}
