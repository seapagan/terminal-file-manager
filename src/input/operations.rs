use super::{Action, Controller};
use crate::{
    operations::{Dialog, Intent, Operation, OperationDialog, Options, Plan},
    ui::Geometry,
};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

pub(super) fn action(controller: &mut Controller, action: Action, geometry: &Geometry) -> bool {
    if action == Action::Jobs {
        controller.app.operations.show_history = !controller.app.operations.show_history;
        return true;
    }
    let operation = match action {
        Action::Copy | Action::CopyQuick => Operation::Copy,
        Action::Move | Action::MoveQuick => Operation::Move,
        Action::Mkdir => Operation::Mkdir,
        Action::Rename => Operation::Rename,
        Action::Trash => Operation::Trash,
        Action::Delete => Operation::Delete,
        Action::Attributes => {
            if cfg!(unix) {
                Operation::Permissions(0)
            } else {
                Operation::ReadOnly(false)
            }
        }
        _ => return false,
    };
    if let Err(error) = open(controller, action, operation, geometry) {
        controller
            .app
            .operations
            .fail(error.to_string(), controller.app.now);
        controller.app.status = "Operation not started — F9 details".into();
    }
    true
}

fn open(
    controller: &mut Controller,
    action: Action,
    operation: Operation,
    geometry: &Geometry,
) -> std::io::Result<()> {
    let pane = controller.app.active;
    let base = controller.app.panes[pane].path.clone();
    let destination = initial_destination(controller, operation);
    let mut plan = if operation == Operation::Mkdir {
        Plan::mkdir(base.clone())
    } else {
        Plan::capture(&controller.app.panes[pane], operation, destination.clone())?
    };
    validate_single(&plan)?;
    capture_target(controller, &mut plan, geometry)?;
    if immediate(controller, action, &plan, geometry)? {
        controller
            .app
            .operations
            .outbox
            .push(Intent::Submit(plan, Options::default()));
        return Ok(());
    }
    let native_target = destination.unwrap_or_else(|| base.clone());
    open_dialog(controller, plan, native_target);
    Ok(())
}

fn open_dialog(controller: &mut Controller, plan: Plan, native_target: std::path::PathBuf) {
    controller.app.operations.scroll = 0;
    let pane = controller.app.active;
    let base = controller.app.panes[pane].path.clone();
    let text = initial_text(&plan, &native_target);
    let destructive = matches!(plan.operation, Operation::Delete | Operation::Trash);
    if destructive {
        controller
            .app
            .operations
            .outbox
            .push(Intent::Preview(plan.clone()));
    }
    controller.app.popup = false;
    controller.app.help = false;
    controller.app.operations.dialog = Some(Dialog::Operation(Box::new(OperationDialog {
        plan,
        pane,
        base,
        native_target,
        text,
        incoming: String::new(),
        edited: false,
        field: 0,
        options: Options::default(),
        ready: !destructive,
        preview_id: None,
        track_scope: true,
        retry_of: None,
        error: None,
    })));
}

fn initial_destination(
    controller: &Controller,
    operation: Operation,
) -> Option<std::path::PathBuf> {
    let pane = controller.app.active;
    match operation {
        Operation::Copy | Operation::Move => Some(controller.app.panes[1 - pane].path.clone()),
        Operation::Mkdir | Operation::Rename => Some(controller.app.panes[pane].path.clone()),
        _ => None,
    }
}

fn capture_target(
    controller: &Controller,
    plan: &mut Plan,
    geometry: &Geometry,
) -> std::io::Result<()> {
    let pane = controller.app.active;
    match plan.operation {
        Operation::Copy | Operation::Move
            if geometry.panes.iter().all(Option::is_some)
                || controller.app.panes[1 - pane].directory.is_some() =>
        {
            plan.capture_destination(&controller.app.panes[1 - pane])?;
        }
        Operation::Mkdir => plan.capture_destination(&controller.app.panes[pane])?,
        _ => {}
    }
    Ok(())
}

fn immediate(
    controller: &Controller,
    action: Action,
    plan: &Plan,
    geometry: &Geometry,
) -> std::io::Result<bool> {
    if !matches!(plan.operation, Operation::Copy | Operation::Move)
        || !geometry.panes.iter().all(Option::is_some)
    {
        return Ok(false);
    }
    if controller.app.panes[1 - controller.app.active]
        .pending
        .is_some()
    {
        return Err(std::io::Error::other(
            "Destination listing pending; wait before approving",
        ));
    }
    Ok(plan.operands.len() == 1 || matches!(action, Action::CopyQuick | Action::MoveQuick))
}

fn validate_single(plan: &Plan) -> std::io::Result<()> {
    if plan.operation == Operation::Rename && plan.operands.len() != 1 {
        return Err(std::io::Error::other(
            "Rename requires exactly one visible entry",
        ));
    }
    if matches!(
        plan.operation,
        Operation::Permissions(_) | Operation::ReadOnly(_)
    ) && plan.operands.iter().any(|entry| {
        !matches!(
            entry.kind,
            crate::fs::EntryKind::File | crate::fs::EntryKind::Directory
        )
    }) {
        return Err(std::io::Error::other(
            "Attribute edits unsupported for links/reparse objects; targets are not followed",
        ));
    }
    Ok(())
}

fn initial_text(plan: &Plan, target: &std::path::Path) -> String {
    match plan.operation {
        Operation::Mkdir => String::new(),
        Operation::Rename => plan.operands[0].name.to_str().unwrap_or("").into(),
        Operation::Permissions(_) => format!("{:04o}", plan.operands[0].mode.unwrap_or(0) & 0o7777),
        Operation::ReadOnly(_) => "true".into(),
        _ => target.to_str().unwrap_or("").into(),
    }
}

pub(super) fn handle(controller: &mut Controller, event: &Event, geometry: &Geometry) -> bool {
    if interrupt(event) {
        exit(controller, false);
        return true;
    }
    if controller.app.diagnostics {
        return false;
    }
    if scroll(controller, event, geometry) {
        return true;
    }
    if modal_click(controller, event, geometry) {
        return true;
    }
    if let Some(dialog) = controller.app.operations.dialog.take() {
        controller.app.operations.dialog =
            super::operation_dialog::handle(controller, dialog, event);
        return true;
    }
    if controller.app.operations.waiting_exit.is_some() {
        if matches!(event, Event::Key(key) if key.code == KeyCode::Esc) {
            controller.app.operations.waiting_exit = None;
            controller.app.operations.exit_snapshot = None;
        }
        return true;
    }
    if editing_surface(controller) {
        return false;
    }
    super::operation_jobs::handle(controller, event, geometry)
}

fn scroll(controller: &mut Controller, event: &Event, geometry: &Geometry) -> bool {
    let state = &mut controller.app.operations;
    if state.dialog.is_none() && !state.show_history && state.conflicts.is_empty() {
        return false;
    }
    let limit = usize::from(geometry.operation_scroll_limit);
    state.scroll = state.scroll.min(limit);
    let delta = match event {
        Event::Key(key) => match key.code {
            KeyCode::PageUp => -8,
            KeyCode::PageDown => 8,
            _ => return false,
        },
        Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::ScrollUp => -3,
            MouseEventKind::ScrollDown => 3,
            _ => return false,
        },
        _ => return false,
    };
    state.scroll = state.scroll.saturating_add_signed(delta).min(limit);
    true
}

fn interrupt(event: &Event) -> bool {
    matches!(event, Event::Key(key)
        if key.kind != KeyEventKind::Release
        && key.code == KeyCode::Char('c')
        && key.modifiers == KeyModifiers::CONTROL)
}

fn editing_surface(controller: &Controller) -> bool {
    controller.filter_before.is_some()
        || controller.app.help
        || controller.app.popup
        || controller.app.view_menu.is_some()
        || controller.app.profile_menu.is_some()
}

fn modal_click(controller: &mut Controller, event: &Event, geometry: &Geometry) -> bool {
    if let Event::Mouse(mouse) = event
        && controller.app.operations.dialog.is_some()
        && mouse.kind == MouseEventKind::Down(MouseButton::Left)
    {
        if let Some(control) = geometry
            .operation_controls
            .iter()
            .find(|control| control.area.contains((mouse.column, mouse.row).into()))
        {
            return activate_control(controller, control.target, geometry);
        }
        return true;
    }
    false
}

pub(super) fn activate_control(
    controller: &mut Controller,
    target: crate::ui::OperationTarget,
    geometry: &Geometry,
) -> bool {
    match target {
        crate::ui::OperationTarget::Key(key) => handle(
            controller,
            &Event::Key(crossterm::event::KeyEvent::new(key, KeyModifiers::NONE)),
            geometry,
        ),
        crate::ui::OperationTarget::Job(index) => {
            let state = &mut controller.app.operations;
            if state.dialog.is_none() && state.show_history && index < state.history.len() {
                state.cursor = index;
                state.scroll = 0;
            }
            true
        }
        crate::ui::OperationTarget::Field(index) => {
            if let Some(Dialog::Operation(dialog)) = &mut controller.app.operations.dialog {
                dialog.field = index.min(1);
            }
            true
        }
    }
}

pub(super) fn exit(controller: &mut Controller, save: bool) {
    super::operation_exit::request(controller, save);
}

pub(crate) fn check_exit(controller: &mut Controller) {
    super::operation_exit::check(controller);
}
