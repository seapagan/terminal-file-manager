use crate::color::Role;
use crate::{
    app::App,
    operations::{Dialog, Event, Operation},
};
use crossterm::event::KeyCode;

pub(super) fn role(app: &App, key: KeyCode) -> Role {
    match (&app.operations.dialog, key) {
        (Some(Dialog::IncidentExit { .. }), KeyCode::F(10))
        | (Some(Dialog::Operation(_)), KeyCode::Char('D')) => Role::Error,
        (Some(Dialog::Operation(_)), KeyCode::Char('T')) => Role::Warning,
        (None, KeyCode::Char('r'))
            if !app.operations.conflicts.is_empty() && !app.operations.show_history =>
        {
            Role::Warning
        }
        _ => Role::Key,
    }
}

/// Presentation mirrors existing domain focus; it never changes Enter authorization.
pub(super) fn focused(app: &App) -> Option<KeyCode> {
    match &app.operations.dialog {
        Some(Dialog::IncidentExit { .. }) => Some(KeyCode::Esc),
        Some(Dialog::Exit { .. }) => Some(KeyCode::Char('r')),
        Some(Dialog::Operation(dialog))
            if matches!(dialog.plan.operation, Operation::Delete | Operation::Trash) =>
        {
            Some(KeyCode::Enter)
        }
        Some(Dialog::Operation(dialog)) if dialog.plan.operation == Operation::Mkdir => {
            Some(KeyCode::Enter)
        }
        None if !app.operations.show_history => app.operations.conflicts.front().map(|conflict| {
            [
                KeyCode::Char('s'),
                KeyCode::Char('r'),
                KeyCode::Char('k'),
                KeyCode::Char('n'),
                KeyCode::Esc,
            ][conflict.choice.min(4)]
        }),
        _ => None,
    }
}

type Button = (&'static str, KeyCode);

pub(super) fn buttons(app: &App, narrow: bool) -> Vec<Button> {
    match &app.operations.dialog {
        Some(Dialog::IncidentExit { typed, .. }) => {
            let mut buttons = vec![("Esc Stay", KeyCode::Esc)];
            if typed == "ACKNOWLEDGE" && !app.operations.active() && !app.operations.unknown() {
                buttons.push((
                    if narrow {
                        "F10 Exit"
                    } else {
                        "F10 Acknowledge and exit"
                    },
                    KeyCode::F(10),
                ));
            }
            buttons
        }
        Some(Dialog::Exit { .. }) => vec![
            ("R Return", KeyCode::Char('r')),
            (
                if narrow { "W Wait" } else { "W Wait then exit" },
                KeyCode::Char('w'),
            ),
            (
                if narrow {
                    "C Stop + exit"
                } else {
                    "C Cancel safely then exit"
                },
                KeyCode::Char('c'),
            ),
        ],
        Some(Dialog::Operation(dialog)) => operation(dialog, narrow),
        None if app.operations.show_history => history(app, narrow),
        None if !app.operations.conflicts.is_empty() => conflict(app, narrow),
        _ => vec![("Esc Return", KeyCode::Esc)],
    }
}

fn operation(dialog: &crate::operations::OperationDialog, narrow: bool) -> Vec<Button> {
    match dialog.plan.operation {
        Operation::Delete | Operation::Trash => destructive(dialog.plan.operation, narrow),
        operation => {
            let mut buttons = vec![
                ("Esc Cancel", KeyCode::Esc),
                (
                    if operation == Operation::Mkdir {
                        "Enter Create"
                    } else {
                        "Enter Run"
                    },
                    KeyCode::Enter,
                ),
            ];
            if matches!(operation, Operation::Copy | Operation::Move) {
                buttons.push((
                    match (narrow, dialog.options.checksum) {
                        (true, false) => "F2 SHA-256 on",
                        (true, true) => "F2 SHA-256 off",
                        (false, false) => "F2 Enable SHA-256",
                        (false, true) => "F2 Disable SHA-256",
                    },
                    KeyCode::F(2),
                ));
            }
            buttons
        }
    }
}

fn destructive(operation: Operation, narrow: bool) -> Vec<Button> {
    let mut buttons = vec![("Enter Cancel", KeyCode::Enter)];
    buttons.push(if operation == Operation::Delete {
        (
            if narrow {
                "D DELETE"
            } else {
                "D DELETE permanently"
            },
            KeyCode::Char('D'),
        )
    } else {
        ("T Trash", KeyCode::Char('T'))
    });
    buttons
}

fn history(app: &App, narrow: bool) -> Vec<Button> {
    let mut buttons = Vec::new();
    if let Some(job) = app.operations.history.get(app.operations.cursor) {
        if app.operations.waiting_exit.is_none() && job.can_reconcile() {
            buttons.push(("X Reconcile", KeyCode::Char('x')));
        }
        if app.operations.waiting_exit.is_none() && job.can_retry() {
            buttons.push((
                if narrow {
                    "R Retry"
                } else {
                    "R Retry incomplete"
                },
                KeyCode::Char('r'),
            ));
        }
        if job.active() {
            buttons.push(("C Cancel job", KeyCode::Char('c')));
        }
        if app
            .operations
            .conflicts
            .iter()
            .any(|conflict| conflict.id == job.id)
        {
            buttons.push(("Enter Conflict", KeyCode::Enter));
        }
    }
    buttons.push(("Esc Close", KeyCode::Esc));
    buttons
}

fn conflict(app: &App, narrow: bool) -> Vec<Button> {
    let mut buttons = vec![("S Skip", KeyCode::Char('s'))];
    if let Some(conflict) = app.operations.conflicts.front()
        && let Event::Conflict {
            source,
            destination,
            newer_available,
        } = &conflict.event
    {
        if source.may_recurse() == destination.may_recurse() {
            buttons.push(("R Replace", KeyCode::Char('r')));
        }
        buttons.push(("K Keep both", KeyCode::Char('k')));
        if *newer_available {
            buttons.push(("N Newer", KeyCode::Char('n')));
        }
    }
    buttons.push(("A All", KeyCode::Char('a')));
    buttons.push((
        if narrow {
            "Esc Cancel"
        } else {
            "Esc Cancel job"
        },
        KeyCode::Esc,
    ));
    buttons
}

/// Model readiness remains independent of layout visibility.
pub(super) fn enabled(app: &App, key: KeyCode) -> bool {
    let Some(Dialog::Operation(dialog)) = &app.operations.dialog else {
        return true;
    };
    match (dialog.plan.operation, key) {
        (Operation::Trash | Operation::Delete, KeyCode::Char('T' | 'D')) => {
            dialog.ready && dialog.plan.preview.is_some() && dialog.error.is_none()
        }
        (Operation::Mkdir, KeyCode::Enter) => dialog.directory_name_valid(),
        _ => true,
    }
}
