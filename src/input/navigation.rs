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
        if parent_action(action, self.app.panes[id].parent_focused()) {
            if let Some(request) = self.app.parent() {
                return vec![request];
            }
            self.app.status = "Filesystem root".into();
            return Vec::new();
        }
        let pane = &self.app.panes[id];
        let path = match action {
            Action::Enter => {
                let Some(entry) = pane.current() else {
                    self.app.status = "No visible entry".into();
                    return Vec::new();
                };
                if !entry.enterable {
                    self.app.status = "The file viewer is not available yet.".into();
                    return Vec::new();
                }
                entry.path.clone()
            }
            _ => pane.path.clone(),
        };
        vec![self.app.request_listing(id, path)]
    }
}

fn parent_action(action: Action, parent_focused: bool) -> bool {
    action == Action::Parent || (action == Action::Enter && parent_focused)
}
