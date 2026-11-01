//! Bound stored scroll to the renderer's actual content and viewport geometry.
use super::{Action, Controller, controller::page_height};
use crate::ui::Geometry;
use crossterm::event::MouseEventKind;

pub(super) fn resize(controller: &mut Controller, width: u16, height: u16) {
    controller.dragging_divider = false;
    let geometry = crate::ui::geometry(
        &controller.app,
        ratatui::layout::Rect::new(0, 0, width, height),
    );
    controller.app.overlay_scroll = controller
        .app
        .overlay_scroll
        .min(geometry.overlay_scroll_limit);
    controller.app.metadata_scroll = controller
        .app
        .metadata_scroll
        .min(geometry.metadata_scroll_limit);
}

pub(super) fn wheel(scroll: &mut u16, kind: MouseEventKind, limit: u16) {
    *scroll = (*scroll).min(limit);
    match kind {
        MouseEventKind::ScrollUp => *scroll = scroll.saturating_sub(1),
        MouseEventKind::ScrollDown => *scroll = scroll.saturating_add(1).min(limit),
        _ => {}
    }
}

impl Controller {
    pub(super) fn movement(&mut self, action: Action, geometry: &Geometry) {
        let height = geometry.help.as_ref().map_or_else(
            || page_height(geometry, self.app.active),
            |layout| usize::from(layout.body.height),
        );
        if self.app.help || self.app.popup {
            self.overlay_movement(action, height, geometry.overlay_scroll_limit);
            return;
        }
        let pane = &mut self.app.panes[self.app.active];
        let delta = match action {
            Action::Up => -1,
            Action::Home => -isize::MAX,
            Action::End => isize::MAX,
            Action::PageUp => -(height as isize),
            Action::PageDown => height as isize,
            _ => 1,
        };
        pane.move_cursor(delta, height);
    }

    fn overlay_movement(&mut self, action: Action, height: usize, limit: u16) {
        let delta = match action {
            Action::Up => -1,
            Action::Home => -i16::MAX,
            Action::End => i16::MAX,
            Action::PageUp => -(height as i16),
            Action::PageDown => height as i16,
            _ => 1,
        };
        self.app.overlay_scroll = self
            .app
            .overlay_scroll
            .min(limit)
            .saturating_add_signed(delta)
            .min(limit);
    }
}
