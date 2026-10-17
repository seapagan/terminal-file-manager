//! Metadata view lifetimes follow visible surfaces and active pane focus.
use super::{Action, Controller};
use crate::{app::Request, ui::Geometry};
use crossterm::event::Event;
use std::time::Duration;

impl Controller {
    /// Normalize terminal input. Clock/geometry are explicit deterministic inputs.
    pub fn handle(&mut self, event: Event, geometry: &Geometry, now: Duration) -> Vec<Request> {
        self.sync_metadata(geometry);
        let width = if let Event::Resize(width, _) = event {
            width
        } else {
            geometry.footer.width
        };
        let requests = self.handle_event(event, geometry, now);
        self.app
            .sync_metadata_views(self.app.sidebar_visible(width));
        requests
    }

    fn sync_metadata(&mut self, geometry: &Geometry) {
        self.app
            .sync_metadata_views(self.app.sidebar_visible(geometry.footer.width));
    }

    /// Keyboard and clickable controls share this dispatcher and availability policy.
    pub fn dispatch(&mut self, action: Action, geometry: &Geometry) -> Vec<Request> {
        self.sync_metadata(geometry);
        let requests = self.dispatch_action(action, geometry);
        self.sync_metadata(geometry);
        requests
    }
}
