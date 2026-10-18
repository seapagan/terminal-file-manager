//! Diagnostics never takes ordinary browsing or modal-navigation keys.
use super::{Action, Controller};
use crate::ui::Geometry;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEvent};

pub(super) fn handle(controller: &mut Controller, event: &Event, geometry: &Geometry) -> bool {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => {
            if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                return false;
            }
            if let Some(
                action @ (Action::Diagnostics | Action::DiagnosticsUp | Action::DiagnosticsDown),
            ) = controller.bindings.action(*key)
            {
                if action != Action::Diagnostics || key.kind == KeyEventKind::Press {
                    controller.dispatch(action, geometry);
                }
                return true;
            }
            false
        }
        Event::Mouse(event) => mouse(controller, *event, geometry),
        _ => false,
    }
}

pub(super) fn action(controller: &mut Controller, action: Action, geometry: &Geometry) {
    if action == Action::Diagnostics {
        controller.app.diagnostics = !controller.app.diagnostics;
        controller.last_click = None;
        controller.dragging_divider = false;
    } else if controller.app.diagnostics {
        let scroll = &mut controller.app.diagnostics_scroll;
        *scroll = (*scroll).min(geometry.diagnostics_scroll_limit);
        *scroll = if action == Action::DiagnosticsUp {
            scroll.saturating_sub(1)
        } else {
            scroll
                .saturating_add(1)
                .min(geometry.diagnostics_scroll_limit)
        };
    }
}

pub(super) fn mouse(controller: &mut Controller, event: MouseEvent, geometry: &Geometry) -> bool {
    if !controller.app.diagnostics
        || !geometry
            .diagnostics
            .is_some_and(|area| area.contains((event.column, event.row).into()))
    {
        return false;
    }
    controller.last_click = None;
    controller.dragging_divider = false;
    super::scroll::wheel(
        &mut controller.app.diagnostics_scroll,
        event.kind,
        geometry.diagnostics_scroll_limit,
    );
    true
}
