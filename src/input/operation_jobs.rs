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
        if plan.operation == crate::operations::Operation::CreateFile
            && let Some(path) = &plan.destination
        {
            self.app.panes[self.app.active].track_creation(id, path.clone());
        }
        self.app.operations.history.push(Job {
            presentation: None,
            retry_eligible: false,
            id,
            plan,
            options,
            state: JobState::Queued,
            record: None,
            progress: None,
            started: self.app.now,
            finished: None,
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
                    dialog.plan = *plan;
                    dialog.ready = true;
                    dialog.error = None;
                    self.app.status = "Preview ready; review exact counts before confirming".into();
                }
            }
            Update::Started { id } => {
                self.start_operation(id);
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
                if self.app.operations.conflicts.is_empty()
                    && !self.app.operations.show_history
                    && self.app.operations.dialog.is_none()
                {
                    self.app.operations.scroll = 0;
                }
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

    fn start_operation(&mut self, id: u64) {
        if let Some(job) = self
            .app
            .operations
            .history
            .iter_mut()
            .find(|job| job.id == id)
        {
            job.state = JobState::Running;
            if job.record.is_none() {
                job.started = self.app.now;
            }
        }
    }

    fn finish_operation(&mut self, id: u64, record: crate::operations::Record) -> Vec<Request> {
        if let Some(Dialog::Operation(dialog)) = &mut self.app.operations.dialog
            && dialog.preview_id == Some(id)
        {
            dialog.ready = false;
            let detail = record
                .entries
                .first()
                .map_or_else(|| "Preview cancelled".into(), |entry| entry.detail.clone());
            dialog.error = Some(detail);
            self.app.status = "Preview unavailable; confirmation disabled".into();
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
            job.finish(record, self.app.now);
            self.app.status = crate::operations::summary(job);
            let severity = match job.state {
                JobState::Completed => crate::operations::NoticeSeverity::Success,
                JobState::Failed | JobState::Unknown | JobState::Critical => {
                    crate::operations::NoticeSeverity::Error
                }
                _ => crate::operations::NoticeSeverity::Warning,
            };
            let incident = job.state == JobState::Critical;
            let success = job.state == JobState::Completed;
            for pane in &mut self.app.panes {
                pane.finish_creation(id, success);
            }
            if !incident {
                self.app
                    .operations
                    .notify(self.app.status.clone(), self.app.now, severity);
            }
            super::operations::check_exit(self);
            return refresh(self);
        }
        Vec::new()
    }
}

fn refresh(controller: &mut Controller) -> Vec<Request> {
    let mut requests = Vec::new();
    for id in 0..2 {
        let pane = &controller.app.panes[id];
        if id != controller.app.active && pane.context == 0 && pane.pending.is_none() {
            continue;
        }
        if let Some(request) = controller.app.request_refresh(id) {
            requests.push(request);
        }
    }
    requests
}

pub(super) fn handle(controller: &mut Controller, event: &Event, geometry: &Geometry) -> bool {
    let owns_input =
        controller.app.operations.show_history || !controller.app.operations.conflicts.is_empty();
    if let Event::Mouse(mouse) = event
        && owns_input
        && mouse.kind == MouseEventKind::Down(MouseButton::Left)
    {
        if let Some(target) = geometry.operation_target(mouse.column, mouse.row) {
            return super::operations::activate_control(controller, target, geometry);
        }
        return true;
    }
    let Event::Key(key) = event else {
        return owns_input;
    };
    if key.kind != KeyEventKind::Press {
        return owns_input;
    }
    if controller.app.operations.show_history {
        history_key(controller, key.code);
        return true;
    }
    if !controller.app.operations.conflicts.is_empty() {
        conflict_key(controller, key.code, geometry);
        return true;
    }
    false
}

fn history_key(controller: &mut Controller, key: KeyCode) {
    let state = &mut controller.app.operations;
    if state.waiting_exit.is_some() && matches!(key, KeyCode::Char('r' | 'R' | 'x' | 'X')) {
        return;
    }
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
        KeyCode::Char('e' | 'E') => {
            state.jobs_evidence = !state.jobs_evidence;
            state.scroll = 0;
        }
        KeyCode::PageUp => state.scroll = state.scroll.saturating_sub(8),
        KeyCode::PageDown => state.scroll = state.scroll.saturating_add(8),
        KeyCode::Char('r' | 'R' | 'c' | 'C') => {
            if let Some(job) = state.history.get(state.cursor) {
                let intent = if matches!(key, KeyCode::Char('r' | 'R')) {
                    if !job.can_retry() {
                        return;
                    }
                    Intent::Retry(job.id)
                } else {
                    if !job.active() {
                        return;
                    }
                    Intent::Cancel(job.id)
                };
                state.outbox.push(intent);
            }
        }
        KeyCode::Char('x' | 'X') => {
            if let Some(job) = state.history.get(state.cursor)
                && job.can_reconcile()
            {
                state.outbox.push(Intent::Reconcile(job.id));
            }
        }
        KeyCode::Enter => open_conflict(state),
        _ => {}
    }
}

fn open_conflict(state: &mut crate::operations::State) {
    if let Some(job) = state.history.get(state.cursor)
        && let Some(index) = state
            .conflicts
            .iter()
            .position(|conflict| conflict.id == job.id)
        && let Some(conflict) = state.conflicts.remove(index)
    {
        state.conflicts.push_front(conflict);
    }
    state.show_history = false;
    state.scroll = 0;
}

fn conflict_key(controller: &mut Controller, key: KeyCode, geometry: &Geometry) {
    if key == KeyCode::F(9) {
        controller.app.operations.show_history = true;
        controller.app.operations.jobs_evidence = false;
        controller.app.operations.scroll = 0;
        return;
    }
    let Some(conflict) = controller.app.operations.conflicts.front_mut() else {
        return;
    };
    let Some(decision) = choose(conflict, key, geometry) else {
        return;
    };
    if let Some(reason) = disabled(&conflict.event, &decision) {
        controller.app.status = reason.into();
        return;
    }
    if !visible_decision(geometry, &decision) {
        controller.app.status = "Resize to review the conflict before replacing.".into();
        return;
    }
    let decision = if conflict.all && !matches!(decision, Decision::Cancel) {
        Decision::All(Box::new(decision))
    } else {
        decision
    };
    let _ = conflict.reply.try_send(decision);
    controller.app.operations.conflicts.pop_front();
    controller.app.operations.scroll = 0;
}

fn choose(conflict: &mut PendingConflict, key: KeyCode, geometry: &Geometry) -> Option<Decision> {
    Some(match key {
        KeyCode::Up | KeyCode::Left => {
            advance(conflict, -1, geometry);
            return None;
        }
        KeyCode::Down | KeyCode::Right => {
            advance(conflict, 1, geometry);
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

fn advance(conflict: &mut PendingConflict, direction: isize, geometry: &Geometry) {
    let mut choice = conflict.choice.min(4);
    loop {
        let next = (choice as isize + direction).clamp(0, 4) as usize;
        if next == choice {
            return;
        }
        choice = next;
        if disabled(&conflict.event, &selected(choice)).is_none()
            && visible_decision(geometry, &selected(choice))
        {
            conflict.choice = choice;
            return;
        }
    }
}

fn visible_decision(geometry: &Geometry, decision: &Decision) -> bool {
    match decision {
        Decision::Replace => geometry.operation_enabled(KeyCode::Char('r')),
        Decision::Newer => geometry.operation_enabled(KeyCode::Char('n')),
        _ => true,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::App, fs, input::Bindings};

    #[test]
    fn operation_refresh_leaves_an_unloaded_inactive_pane_unscheduled() {
        let root = tempfile::tempdir().unwrap();
        let snapshot =
            fs::list(root.path(), &fs::IgnoreContext::isolated(), &false.into()).unwrap();
        let mut app = App::new(root.path().into());
        app.complete_listing(0, 1, Ok(snapshot.clone()));
        let mut controller = Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        );
        let requests = refresh(&mut controller);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].pane, 0);
        controller.app.complete_listing(1, 1, Ok(snapshot));
        assert!(controller.app.take_refresh(1).is_none());
    }

    #[test]
    fn operation_refresh_retries_a_loaded_but_unverified_inactive_context() {
        let root = tempfile::tempdir().unwrap();
        let mut snapshot =
            fs::list(root.path(), &fs::IgnoreContext::isolated(), &false.into()).unwrap();
        snapshot.directory = None;
        let mut app = App::new(root.path().into());
        app.complete_listing(1, 1, Ok(snapshot));
        let mut controller = Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        );
        let requests = refresh(&mut controller);
        assert!(requests.iter().any(|request| request.pane == 1));
        assert!(controller.app.panes[1].error.is_some());
    }
}
