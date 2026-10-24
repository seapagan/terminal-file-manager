use crate::{
    app::App,
    operations::{Dialog, Event, Operation},
};
use crossterm::event::KeyCode;

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
        Operation::Delete | Operation::Trash => {
            let mut buttons = vec![("Enter Cancel", KeyCode::Enter)];
            if dialog.ready {
                buttons.push(if dialog.plan.operation == Operation::Delete {
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
            }
            buttons
        }
        operation => {
            let mut buttons = vec![("Enter Run", KeyCode::Enter), ("Esc Cancel", KeyCode::Esc)];
            if matches!(operation, Operation::Copy | Operation::Move) {
                buttons.push(("F2 SHA-256", KeyCode::F(2)));
            }
            buttons
        }
    }
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
