use super::Controller;
use crate::{
    app::Request,
    operations::{
        Decision, Dialog, Event as JobEvent, Intent, Job, JobState, PendingConflict, Update,
    },
    ui::Geometry,
};
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};

impl Controller {
    /// Install a job identity after the queue accepts exactly this frozen plan.
    pub fn register_operation(
        &mut self,
        id: u64,
        plan: crate::operations::Plan,
        options: crate::operations::Options,
        retry_of: Option<u64>,
    ) {
        self.app.operations.history.push(Job {
            id,
            plan,
            options,
            state: JobState::Queued,
            record: None,
            progress: None,
            started: self.app.now,
            retry_of,
        });
        self.app.operations.cursor = self.app.operations.history.len() - 1;
        self.app.status = format!("Job {id} queued — F9 / Alt+J for jobs");
    }

    /// Worker outcomes remain tied to their job even after pane navigation changes.
    pub fn complete_operation(&mut self, update: Update) -> Vec<Request> {
        match update {
            Update::Preview { id, plan } => {
                if let Some(Dialog::Operation(dialog)) = &mut self.app.operations.dialog
                    && dialog.preview_id == Some(id)
                {
                    dialog.plan = plan;
                    dialog.ready = true;
                    self.app.status = "Preview ready; review exact counts before confirming".into();
                }
            }
            Update::Started { id } => {
                if let Some(job) = self
                    .app
                    .operations
                    .history
                    .iter_mut()
                    .find(|job| job.id == id)
                {
                    job.state = JobState::Running;
                    job.started = self.app.now;
                }
            }
            Update::Progress { id, event } => {
                if let Some(job) = self
                    .app
                    .operations
                    .history
                    .iter_mut()
                    .find(|job| job.id == id)
                {
                    job.progress = Some(event);
                }
            }
            Update::Conflict { id, event, reply } => {
                self.app.operations.conflicts.push_back(PendingConflict {
                    id,
                    event,
                    reply,
                    all: false,
                    choice: 0,
                });
            }
            Update::Finished { id, record } => {
                return self.finish_operation(id, record);
            }
        }
        Vec::new()
    }

    fn finish_operation(&mut self, id: u64, record: crate::operations::Record) -> Vec<Request> {
        if let Some(Dialog::Operation(dialog)) = &mut self.app.operations.dialog
            && dialog.preview_id == Some(id)
        {
            dialog.ready = false;
            self.app.status = record
                .entries
                .first()
                .map_or_else(|| "Preview cancelled".into(), |entry| entry.detail.clone());
            return Vec::new();
        }
        self.app
            .operations
            .conflicts
            .retain(|conflict| conflict.id != id);
        if let Some(job) = self
            .app
            .operations
            .history
            .iter_mut()
            .find(|job| job.id == id)
        {
            job.finish(record);
            self.app.status = crate::operations::summary(job);
            let warning = job.state != JobState::Completed;
            self.app
                .operations
                .notify(self.app.status.clone(), self.app.now, warning);
            super::operations::check_exit(self);
            return refresh(self);
        }
        Vec::new()
    }
}

fn refresh(controller: &mut Controller) -> Vec<Request> {
    let mut requests = Vec::new();
    for id in 0..2 {
        if controller.app.panes[id].pending.is_none() {
            requests.push(
                controller
                    .app
                    .request_listing(id, controller.app.panes[id].path.clone()),
            );
        }
    }
    requests
}

pub(super) fn handle(controller: &mut Controller, event: &Event, geometry: &Geometry) -> bool {
    if let Event::Mouse(mouse) = event
        && (controller.app.operations.show_history
            || !controller.app.operations.conflicts.is_empty())
        && mouse.kind == MouseEventKind::Down(MouseButton::Left)
    {
        if let Some(control) = geometry
            .operation_controls
            .iter()
            .find(|control| control.area.contains((mouse.column, mouse.row).into()))
        {
            return handle(
                controller,
                &Event::Key(crossterm::event::KeyEvent::new(
                    control.key,
                    crossterm::event::KeyModifiers::NONE,
                )),
                geometry,
            );
        }
        return true;
    }
    let Event::Key(key) = event else {
        return controller.app.operations.show_history
            || !controller.app.operations.conflicts.is_empty();
    };
    if key.kind != KeyEventKind::Press {
        return false;
    }
    if controller.app.operations.show_history {
        history_key(controller, key.code);
        return true;
    }
    if !controller.app.operations.conflicts.is_empty() {
        conflict_key(controller, key.code);
        return true;
    }
    false
}

fn history_key(controller: &mut Controller, key: KeyCode) {
    let state = &mut controller.app.operations;
    match key {
        KeyCode::Esc | KeyCode::F(9) => state.show_history = false,
        KeyCode::Up | KeyCode::Char('k') => {
            state.cursor = state.cursor.saturating_sub(1);
            state.scroll = 0;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.cursor = (state.cursor + 1).min(state.history.len().saturating_sub(1));
            state.scroll = 0;
        }
        KeyCode::PageUp => state.scroll = state.scroll.saturating_sub(8),
        KeyCode::PageDown => state.scroll = state.scroll.saturating_add(8),
        KeyCode::Char('r' | 'R' | 'c' | 'C') => {
            if let Some(job) = state.history.get(state.cursor) {
                let intent = if matches!(key, KeyCode::Char('r' | 'R')) {
                    Intent::Retry(job.id)
                } else {
                    Intent::Cancel(job.id)
                };
                state.outbox.push(intent);
            }
        }
        KeyCode::Enter => state.show_history = false,
        _ => {}
    }
}

fn conflict_key(controller: &mut Controller, key: KeyCode) {
    if key == KeyCode::F(9) {
        controller.app.operations.show_history = true;
        return;
    }
    let Some(conflict) = controller.app.operations.conflicts.front_mut() else {
        return;
    };
    let Some(decision) = choose(conflict, key) else {
        return;
    };
    if let Some(reason) = disabled(&conflict.event, &decision) {
        controller.app.status = reason.into();
        return;
    }
    let decision = if conflict.all && !matches!(decision, Decision::Cancel) {
        Decision::All(Box::new(decision))
    } else {
        decision
    };
    let _ = conflict.reply.try_send(decision);
    controller.app.operations.conflicts.pop_front();
}

fn choose(conflict: &mut PendingConflict, key: KeyCode) -> Option<Decision> {
    Some(match key {
        KeyCode::Up | KeyCode::Left => {
            conflict.choice = conflict.choice.saturating_sub(1);
            return None;
        }
        KeyCode::Down | KeyCode::Right => {
            conflict.choice = (conflict.choice + 1).min(4);
            return None;
        }
        KeyCode::Char('a' | 'A') => {
            conflict.all = !conflict.all;
            return None;
        }
        KeyCode::Char('s' | 'S') => Decision::Skip,
        KeyCode::Char('r' | 'R') => Decision::Replace,
        KeyCode::Char('k' | 'K') => Decision::KeepBoth,
        KeyCode::Char('n' | 'N') => Decision::Newer,
        KeyCode::Esc => Decision::Cancel,
        KeyCode::Enter => selected(conflict.choice),
        _ => return None,
    })
}

fn disabled(event: &JobEvent, decision: &Decision) -> Option<&'static str> {
    if let JobEvent::Conflict {
        source,
        destination,
        newer_available,
    } = event
    {
        if matches!(decision, Decision::Newer) && !newer_available {
            return Some("Newer-only unavailable for these timestamps/types");
        }
        if matches!(decision, Decision::Replace)
            && source.may_recurse() != destination.may_recurse()
        {
            return Some("Atomic type replacement unsupported; choose Keep both or Skip");
        }
    }
    None
}

fn selected(choice: usize) -> Decision {
    match choice {
        1 => Decision::Replace,
        2 => Decision::KeepBoth,
        3 => Decision::Newer,
        4 => Decision::Cancel,
        _ => Decision::Skip,
    }
}
