//! Entry-row clicks select and navigate independently of the displayed columns.
use super::{Action, Controller, controller::page_height};
use crate::{app::Request, ui::Geometry};
use crossterm::event::{KeyModifiers, MouseEvent};
use std::time::Duration;

impl Controller {
    pub(super) fn click(
        &mut self,
        id: usize,
        event: MouseEvent,
        geometry: &Geometry,
        now: Duration,
    ) -> Vec<Request> {
        if let Some(requests) = super::path_navigation::header(self, event, geometry) {
            return requests;
        }
        self.app.active = id;
        self.app.sidebar = self.app.panes[id].sidebar;
        let area = geometry.panes[id].expect("hit pane");
        let row = event.row;
        if row < area.y + 2 || row >= area.y + 2 + geometry.listing_rows[id] as u16 {
            return Vec::new();
        }
        let pane = &mut self.app.panes[id];
        let index = pane.viewport_start(page_height(geometry, id)) + usize::from(row - area.y - 2);
        let Some(path) = pane.row_path(index) else {
            return Vec::new();
        };
        if event.modifiers.contains(KeyModifiers::SHIFT) {
            if Some(path.as_path()) == pane.path.parent() {
                pane.cursor = Some(path);
                pane.reset_range();
            } else {
                pane.select_range_to(path, page_height(geometry, id));
            }
            self.last_click = None;
            return Vec::new();
        }
        pane.cursor = Some(path.clone());
        pane.anchor_range();
        let double = self.record_click(id, path, now);
        if double {
            self.dispatch(Action::Enter, geometry)
        } else {
            Vec::new()
        }
    }
    fn record_click(&mut self, id: usize, path: std::path::PathBuf, now: Duration) -> bool {
        let double = self
            .last_click
            .as_ref()
            .is_some_and(|(last_id, last_path, time)| {
                *last_id == id
                    && last_path == &path
                    && now.saturating_sub(*time) <= Duration::from_millis(400)
            });
        self.last_click = if double { None } else { Some((id, path, now)) };
        double
    }
}
