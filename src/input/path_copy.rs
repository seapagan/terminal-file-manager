use super::{Action, Controller};
use crate::{
    path_copy,
    ui::Geometry,
    workflow::{Input, Progress, Target},
};
use crossterm::event::{Event, KeyEventKind, MouseButton, MouseEventKind};

pub(super) fn eligible(c: &Controller) -> bool {
    let app = &c.app;
    !editing(c)
        && !app.notifications.open
        && app.operations.dialog.is_none()
        && !app.operations.show_history
        && app.operations.conflicts.is_empty()
        && app.operations.waiting_exit.is_none()
}

fn editing(c: &Controller) -> bool {
    let app = &c.app;
    c.filter_before.is_some()
        || app.help
        || app.popup
        || app.diagnostics
        || app.context_help.is_some()
        || app.pattern.is_some()
        || app.path_picker.is_some()
        || app.profile_menu.is_some()
        || app.view_menu.is_some()
}

impl Controller {
    /// Retire obsolete footer ownership before drawing or routing the next event.
    pub fn reconcile_footer(&mut self) -> bool {
        let stale = self.app.footer_workflow.as_ref().is_some_and(|flow| {
            !eligible(self)
                || flow
                    .context()
                    .is_none_or(|snapshot| !snapshot.matches_context(&self.app))
        });
        if stale {
            self.app.footer_workflow = None;
        }
        stale
    }
}

pub(super) fn action(c: &mut Controller, action: Action) -> bool {
    if action != Action::PathCopy {
        return false;
    }
    if eligible(c) && c.app.footer_workflow.is_none() {
        activate(c, Input::Click(Target::Choice("path-copy")));
    }
    true
}

fn activate(c: &mut Controller, trigger: Input) {
    match path_copy::open(&c.app, trigger) {
        Ok(flow) => {
            c.app.footer_workflow = Some(flow);
            c.last_click = None;
            c.dragging_divider = false;
        }
        Err(error) => c.app.status = error.into(),
    }
}

pub(super) fn handle(c: &mut Controller, event: &Event, g: &Geometry) -> bool {
    if c.app.footer_workflow.is_none() {
        return activation(c, event);
    }
    if c.reconcile_footer() {
        return false;
    }
    let input = match event {
        Event::Key(key) => Some(Input::Key(*key)),
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => g
            .workflow
            .as_ref()
            .and_then(|layout| layout.hit(mouse.column, mouse.row))
            .map(Input::Click),
        _ => None,
    };
    if let Some(input) = input {
        transition(c, input);
    }
    true
}

fn activation(c: &mut Controller, event: &Event) -> bool {
    if let Event::Key(key) = event
        && key.kind == KeyEventKind::Press
        && eligible(c)
        && c.bindings.action(*key) == Some(Action::PathCopy)
    {
        activate(c, Input::Key(*key));
        return true;
    }
    false
}

fn transition(c: &mut Controller, input: Input) {
    let result = c
        .app
        .footer_workflow
        .as_mut()
        .expect("active workflow")
        .input(input);
    match result {
        Progress::Complete(done) => {
            c.app.footer_workflow = None;
            match path_copy::complete(done, &mut c.app) {
                Ok(path_copy::Outcome::Copy(request)) => {
                    c.app.status = format!("Clipboard request ready: {}", request.scope).into();
                    c.path_copy_request = Some(request);
                }
                Ok(path_copy::Outcome::BaseSet(path)) => {
                    c.app.status = format!("Relative-path base set: {}", path.display()).into();
                }
                Err(error) => c.app.notify(crate::notifications::Severity::Error, error),
            }
        }
        Progress::Cancelled | Progress::Inactive => c.app.footer_workflow = None,
        Progress::Disabled(reason) => c.app.status = reason.into(),
        Progress::Active => {}
    }
}
