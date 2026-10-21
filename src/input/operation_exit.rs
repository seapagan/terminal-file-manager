use super::Controller;
use crate::operations::{Dialog, Intent};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

pub(super) fn request(controller: &mut Controller, save: bool) {
    controller.app.operations.waiting_exit = None;
    controller.app.operations.scroll = 0;
    controller.app.operations.exit_snapshot =
        save.then(|| crate::session::NavigationSession::capture(&controller.app));
    if controller.app.operations.active() || controller.app.operations.unknown() {
        controller.app.operations.dialog = Some(Dialog::Exit { save });
    } else {
        stopped(controller, save);
    }
}

pub(super) fn check(controller: &mut Controller) {
    let Some(save) = controller.app.operations.waiting_exit else {
        return;
    };
    if controller.app.operations.active() {
        return;
    }
    controller.app.operations.waiting_exit = None;
    stopped(controller, save);
}

fn stopped(controller: &mut Controller, save: bool) {
    if controller.app.operations.unknown() {
        controller.app.operations.exit_snapshot = None;
        controller.app.status = "Outcome unknown; exit blocked pending reconciliation".into();
    } else if controller.app.operations.incidents() {
        controller.app.operations.scroll = 0;
        controller.app.operations.dialog = Some(Dialog::IncidentExit {
            save,
            typed: String::new(),
        });
    } else {
        controller.save_session = save;
        controller.quit = true;
    }
}

fn stay(controller: &mut Controller) -> Option<Dialog> {
    controller.app.operations.waiting_exit = None;
    controller.app.operations.exit_snapshot = None;
    None
}

pub(super) fn choices(controller: &mut Controller, save: bool, event: &Event) -> Option<Dialog> {
    if let Event::Key(key) = event
        && key.kind == KeyEventKind::Press
    {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('r' | 'R') => return stay(controller),
            KeyCode::Char('w' | 'W' | 'c' | 'C') => {
                if matches!(key.code, KeyCode::Char('c' | 'C')) {
                    let state = &mut controller.app.operations;
                    state.outbox.extend(
                        state
                            .history
                            .iter()
                            .filter(|job| job.active())
                            .map(|job| Intent::Cancel(job.id)),
                    );
                }
                controller.app.operations.waiting_exit = Some(save);
                check(controller);
                return controller.app.operations.dialog.take();
            }
            _ => {}
        }
    }
    Some(Dialog::Exit { save })
}

pub(super) fn incident(
    controller: &mut Controller,
    save: bool,
    mut typed: String,
    event: &Event,
) -> Option<Dialog> {
    if let Event::Key(key) = event
        && key.kind == KeyEventKind::Press
    {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => return stay(controller),
            KeyCode::F(10) if ready(controller, &typed, key.modifiers) => {
                controller.save_session = save;
                controller.quit = true;
                return None;
            }
            KeyCode::Backspace => {
                typed.pop();
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && typed.len() < 11 =>
            {
                typed.push(c)
            }
            _ => {}
        }
    }
    Some(Dialog::IncidentExit { save, typed })
}

fn ready(controller: &Controller, typed: &str, modifiers: KeyModifiers) -> bool {
    typed == "ACKNOWLEDGE"
        && modifiers.is_empty()
        && !controller.app.operations.active()
        && !controller.app.operations.unknown()
        && controller.app.operations.incidents()
}
