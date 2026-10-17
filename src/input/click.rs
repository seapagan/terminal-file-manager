//! Entry-row clicks share the listing's owner-cell geometry.
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
        self.app.active = id;
        self.app.sidebar = self.app.panes[id].sidebar;
        let area = geometry.panes[id].expect("hit pane");
        let row = event.row;
        if row < area.y + 2 || row >= area.y + 2 + geometry.listing_rows[id] as u16 {
            return Vec::new();
        }
        let pane = &mut self.app.panes[id];
        let index = pane.viewport_start(page_height(geometry, id)) + usize::from(row - area.y - 2);
        let Some(entry) = pane.visible.get(index).map(|i| &pane.entries[*i]) else {
            return Vec::new();
        };
        let path = entry.path.clone();
        if event.modifiers.contains(KeyModifiers::SHIFT) {
            pane.select_range_to(path, page_height(geometry, id));
            self.last_click = None;
            return Vec::new();
        }
        pane.cursor = Some(path.clone());
        pane.anchor_range();
        if crate::ui::owner_column(pane, area, geometry.listing_rows[id])
            .is_some_and(|cell| cell.contains((event.column, row).into()))
        {
            self.last_click = None;
            return self.dispatch(Action::OwnerDisplay, geometry);
        }
        let double = self
            .last_click
            .as_ref()
            .is_some_and(|(last_id, last_path, time)| {
                *last_id == id
                    && last_path == &path
                    && now.saturating_sub(*time) <= Duration::from_millis(400)
            });
        self.last_click = if double { None } else { Some((id, path, now)) };
        if double {
            self.dispatch(Action::Enter, geometry)
        } else {
            Vec::new()
        }
    }
}
