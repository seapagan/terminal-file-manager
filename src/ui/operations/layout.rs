use super::{OperationControl, OperationTarget, actions, content};
use crate::{
    app::App,
    ui::modal::{self, actions::Action},
};
use ratatui::layout::Rect;

#[derive(Clone, Debug)]
pub(in crate::ui) struct Layout {
    pub outer: Rect,
    pub body: Rect,
    pub list: Rect,
    pub buttons: Vec<OperationControl>,
    pub shell: modal::Layout<OperationTarget>,
    pub(super) rows: Vec<content::Row>,
}

impl Layout {
    pub fn new(app: &App, terminal: Rect) -> Self {
        let rows = content::rows(app);
        let buttons: Vec<_> = actions::buttons(app, terminal.width < 44)
            .into_iter()
            .map(|(label, key)| {
                let mut action = Action::custom(label, OperationTarget::Key(key), key);
                action.role = actions::role(app, key);
                action
            })
            .collect();
        let spec = modal::Spec {
            title: content::title(app),
            content_width: desired_width(&rows, terminal.width),
            pinned_rows: list_rows(app, terminal.height),
            actions: &buttons,
        };
        let mut wrapped = Vec::new();
        let shell = spec.measure(terminal, |width| {
            wrapped = content::wrapped(rows, width);
            wrapped.len()
        });
        Self {
            outer: shell.outer,
            body: shell.body,
            list: shell.pinned,
            buttons: shell
                .actions
                .iter()
                .map(|button| OperationControl {
                    area: button.area,
                    target: button.action.intent,
                    label: button.action.label,
                })
                .collect(),
            shell,
            rows: wrapped,
        }
    }

    pub fn limit(&self, rows: usize) -> usize {
        self.shell.scroll_limit(rows)
    }
}

fn desired_width(rows: &[content::Row], terminal_width: u16) -> usize {
    let mut widest = 0;
    for row in rows {
        widest = widest.max(row.width());
        if widest >= usize::from(terminal_width) {
            break;
        }
    }
    widest
}

fn list_rows(app: &App, height: u16) -> u16 {
    if app.operations.show_history && app.operations.dialog.is_none() {
        app.operations
            .history
            .len()
            .min(5)
            .min(usize::from(height / 4)) as u16
    } else {
        0
    }
}
