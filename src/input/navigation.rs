//! Existing browser navigation shared by keyboard and virtual parent activation.
use super::{Action, Controller};
use crate::app::Request;

impl Controller {
    pub(super) fn navigate(&mut self, action: Action) -> Vec<Request> {
        let id = self.app.active;
        let request = match action {
            Action::Refresh => self.app.request_refresh(id),
            Action::Back => self.app.history(false),
            Action::Forward => self.app.history(true),
            Action::Parent => {
                let request = self.app.parent();
                if request.is_none() {
                    self.app.status = "Filesystem root".into();
                }
                request
            }
            Action::Enter => return self.enter_row(id),
            _ => unreachable!(),
        };
        request.into_iter().collect()
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
