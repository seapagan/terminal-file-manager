//! Bound stored scroll to the renderer's actual content and viewport geometry.
use super::Controller;
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
