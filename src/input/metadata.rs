//! Metadata view lifetimes follow visible surfaces and active pane focus.
use super::{Action, Controller};
use crate::{
    app::{App, Request},
    ui::Geometry,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use std::time::Duration;

impl Controller {
    /// Normalize terminal input. Clock/geometry are explicit deterministic inputs.
    pub fn handle(&mut self, event: Event, geometry: &Geometry, now: Duration) -> Vec<Request> {
        self.app.notifications.advance(now);
        if super::notifications::handle(self, &event, geometry) {
            return Vec::new();
        }
        let closing = close_hit(&event, geometry.modal_close);
        if closing && metadata_owns_close(&self.app, geometry) {
            return self.dispatch(Action::Cancel, geometry);
        }
        let event = if closing {
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        } else {
            event
        };
        self.sync_metadata(geometry);
        let width = if let Event::Resize(width, _) = event {
            width
        } else {
            geometry.footer.width
        };
        let requests = if super::help::context_handle(self, &event, geometry)
            || super::help::handle(self, &event, geometry)
        {
            Vec::new()
        } else {
            self.handle_event(event, geometry, now)
        };
        self.app
            .sync_metadata_views(self.app.sidebar_visible(width));
        requests
    }

    fn sync_metadata(&mut self, geometry: &Geometry) {
        self.app
            .sync_metadata_views(self.app.sidebar_visible(geometry.footer.width));
    }

    pub(super) fn preferred_action(&self, action: Action) -> Action {
        if action != Action::Info {
            return action;
        }
        if self.app.popup {
            return Action::Metadata;
        }
        match self.preferences.metadata_presentation {
            crate::settings::MetadataPresentation::Sidebar => Action::ToggleSidebar,
            crate::settings::MetadataPresentation::Popup => Action::Metadata,
        }
    }

    /// Keyboard and clickable controls share this dispatcher and availability policy.
    pub fn dispatch(&mut self, action: Action, geometry: &Geometry) -> Vec<Request> {
        self.sync_metadata(geometry);
        let requests = self.dispatch_action(action, geometry);
        self.sync_metadata(geometry);
        requests
    }
}

/// The floating metadata surface owns focus; only its controls and global exit remain active.
pub(super) fn accepts(action: Action) -> bool {
    matches!(
        action,
        Action::Up
            | Action::Down
            | Action::Home
            | Action::End
            | Action::PageUp
            | Action::PageDown
            | Action::Count
            | Action::Cancel
            | Action::Metadata
            | Action::Info
            | Action::Help
            | Action::Quit
            | Action::SaveQuit
            | Action::Permissions
            | Action::OwnerDisplay
            | Action::FuzzyTimestamps
            | Action::MetadataPresentation
            | Action::Diagnostics
    )
}

// Metadata's Close action bypasses configurable Esc, just like its existing footer button.
fn metadata_owns_close(app: &App, geometry: &Geometry) -> bool {
    app.popup
        && geometry.context_help.is_none()
        && geometry.help.is_none()
        && geometry.pattern.is_none()
}

fn close_hit(event: &Event, close: Option<ratatui::layout::Rect>) -> bool {
    matches!(event, Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left)
        && close.is_some_and(|area| area.contains((mouse.column, mouse.row).into())))
}
