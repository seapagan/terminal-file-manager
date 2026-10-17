use super::Controller;
use crate::ui::Geometry;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

pub(super) fn adjust(controller: &mut Controller, percent: u16) {
    let percent = percent.clamp(20, 80);
    if controller.app.split_percent != percent {
        controller.app.split_percent = percent;
        controller.preferences.split_percent = percent;
        controller.preferences_changed = true;
    }
}

pub(super) fn mouse(controller: &mut Controller, event: MouseEvent, geometry: &Geometry) -> bool {
    let Some(divider) = geometry.divider else {
        controller.dragging_divider = false;
        return false;
    };
    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            controller.dragging_divider = divider.contains((event.column, event.row).into());
            controller.dragging_divider
        }
        MouseEventKind::Up(MouseButton::Left) => {
            let was_dragging = controller.dragging_divider;
            controller.dragging_divider = false;
            was_dragging
        }
        MouseEventKind::Drag(MouseButton::Left) if controller.dragging_divider => {
            let left = geometry.panes[0].unwrap();
            let right = geometry.panes[1].unwrap();
            let width = right.right().saturating_sub(left.x).max(1);
            let percent = (u32::from(event.column.saturating_sub(left.x)) * 100 / u32::from(width))
                .min(100) as u16;
            adjust(controller, percent);
            true
        }
        _ => false,
    }
}
