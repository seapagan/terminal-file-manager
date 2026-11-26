//! Existing browser navigation shared by keyboard and virtual parent activation.
use super::{Action, Controller};
use crate::app::Request;

impl Controller {
    pub(super) fn navigate(&mut self, action: Action) -> Vec<Request> {
        let id = self.app.active;
        if action == Action::Refresh {
            return self.app.request_refresh(id).into_iter().collect();
        }
        if action == Action::Back {
            return self.app.history(false).into_iter().collect();
        }
        if action == Action::Forward {
            return self.app.history(true).into_iter().collect();
        }
        if action == Action::Parent {
            if self.app.panes[id].path.parent().is_none() {
                self.app.status = "Filesystem root".into();
            }
            return self.app.parent().into_iter().collect();
        }
        if action == Action::Enter {
            return self.enter_row(id);
        }
        vec![
            self.app
                .request_listing(id, self.app.panes[id].path.clone()),
        ]
    }

    fn enter_row(&mut self, id: usize) -> Vec<Request> {
        let pane = &self.app.panes[id];
        if pane.parent_focused() {
            return self.app.parent().into_iter().collect();
        }
        let Some(entry) = pane.current() else {
            self.app.status = "No visible entry".into();
            return Vec::new();
        };
        if !entry.enterable {
            self.app.status = "The file viewer is not available yet.".into();
            return Vec::new();
        }
        vec![self.app.request_listing(id, entry.path.clone())]
    }
}
