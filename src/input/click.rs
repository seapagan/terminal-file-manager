//! Entry-row clicks select and navigate independently of the displayed columns.
use super::{Action, Controller, controller::page_height};
use crate::{app::Request, ui::Geometry};
use crossterm::event::{KeyModifiers, MouseEvent};
use std::{path::PathBuf, time::Duration};

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
        if super::policy_menu::open_at(self, event, geometry) {
            return Vec::new();
        }
        self.app.active = id;
        self.app.sidebar = self.app.panes[id].sidebar;
        let area = geometry.panes[id].expect("hit pane");
        let row = event.row;
        if row < area.y + 2 || row >= area.y + 2 + geometry.listing_rows[id] as u16 {
            return Vec::new();
        }
        let path = self.focus_listing_row(
            id,
            usize::from(row - area.y - 2),
            page_height(geometry, id),
            event.modifiers.contains(KeyModifiers::SHIFT),
        );
        if path.is_some_and(|path| self.record_click(id, path, now)) {
            self.dispatch(Action::Enter, geometry)
        } else {
            Vec::new()
        }
    }

    /// Focus a visible row and return its native path if it can be double-clicked.
    fn focus_listing_row(
        &mut self,
        id: usize,
        offset: usize,
        height: usize,
        extend: bool,
    ) -> Option<PathBuf> {
        let pane = &mut self.app.panes[id];
        let start = pane.viewport_start(height);
        let index = start + offset;
        if index >= pane.row_count() {
            if pane.is_remote() {
                self.last_click = None;
            }
            return None;
        }
        pane.scroll = start;
        let path = pane.row_path(index);

        if extend && let Some(path) = path {
            if Some(path.as_path()) == pane.path.parent() {
                pane.focus_row(index);
                pane.reset_range();
            } else {
                pane.select_range_to(path, height);
            }
            self.last_click = None;
            return None;
        }
        pane.focus_row(index);
        if path.is_some() {
            pane.anchor_range();
        } else {
            self.last_click = None;
        }
        path
    }

    fn record_click(&mut self, id: usize, path: PathBuf, now: Duration) -> bool {
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
