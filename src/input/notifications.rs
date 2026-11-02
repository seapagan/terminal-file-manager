//! The history overlay owns input without changing its underlying pane or Jobs state.
use super::Controller;
use crate::ui::Geometry;
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};

pub(super) fn handle(c: &mut Controller, event: &Event, g: &Geometry) -> bool {
    if super::operations::interrupt(event) {
        c.app.notifications.open = false;
        return false;
    }
    if c.app.notifications.open {
        history(c, event, g);
        return true;
    }
    if let Event::Mouse(mouse) = event
        && let Some(toast) = g
            .toasts
            .iter()
            .find(|t| t.area.contains((mouse.column, mouse.row).into()))
    {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            c.app.notifications.open = true;
            c.app.notifications.cursor = toast.event;
            c.app.notifications.scroll = 0;
        }
        return true;
    }
    false
}

pub(super) fn open(c: &mut Controller) {
    c.app.notifications.open = true;
    c.app.notifications.cursor = c.app.notifications.latest_index().unwrap_or(0);
    c.app.notifications.scroll = 0;
}

fn history(c: &mut Controller, event: &Event, g: &Geometry) {
    let limit = g.notification_history.as_ref().map_or(0, |l| l.limit);
    c.app.notifications.scroll = c.app.notifications.scroll.min(limit);
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => history_key(c, key.code, limit),
        Event::Mouse(mouse) => history_mouse(c, mouse, g, limit),
        Event::Resize(width, height) => {
            super::scroll::resize(c, *width, *height);
            let next =
                crate::ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, *width, *height));
            c.app.notifications.scroll = c
                .app
                .notifications
                .scroll
                .min(next.notification_history.map_or(0, |layout| layout.limit));
        }
        _ => {}
    }
}

fn history_mouse(
    c: &mut Controller,
    mouse: &crossterm::event::MouseEvent,
    g: &Geometry,
    limit: usize,
) {
    let Some(layout) = &g.notification_history else {
        return;
    };
    if mouse.kind == MouseEventKind::Down(MouseButton::Left)
        && let Some(key) = layout.shell.hit(mouse.column, mouse.row)
    {
        history_key(c, *key, limit);
        return;
    }
    if layout
        .shell
        .close
        .is_some_and(|r| r.contains((mouse.column, mouse.row).into()))
        && mouse.kind == MouseEventKind::Down(MouseButton::Left)
    {
        c.app.notifications.open = false;
    } else if layout.shell.body.contains((mouse.column, mouse.row).into()) {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                c.app.notifications.scroll = c.app.notifications.scroll.saturating_sub(3)
            }
            MouseEventKind::ScrollDown => {
                c.app.notifications.scroll = (c.app.notifications.scroll + 3).min(limit)
            }
            _ => {}
        }
    }
}

fn history_key(c: &mut Controller, key: KeyCode, limit: usize) {
    let state = &mut c.app.notifications;
    let previous = state.cursor;
    match key {
        KeyCode::Esc => state.open = false,
        KeyCode::Up => state.cursor = state.cursor.saturating_sub(1),
        KeyCode::Down => {
            state.cursor = (state.cursor + 1).min(state.history.len().saturating_sub(1))
        }
        KeyCode::Home => state.cursor = 0,
        KeyCode::End => state.cursor = state.history.len().saturating_sub(1),
        KeyCode::PageUp => state.scroll = state.scroll.saturating_sub(8),
        KeyCode::PageDown => state.scroll = (state.scroll + 8).min(limit),
        KeyCode::Enter => open_job(c),
        _ => {}
    }
    if previous != c.app.notifications.cursor {
        c.app.notifications.scroll = 0;
    }
}

pub(super) fn status_click(c: &mut Controller, event: &Event, g: &Geometry) -> bool {
    if let Event::Mouse(mouse) = event
        && mouse.kind == MouseEventKind::Down(MouseButton::Left)
        && let Some((_, action)) = g
            .status_controls
            .iter()
            .find(|(area, _)| area.contains((mouse.column, mouse.row).into()))
    {
        c.dispatch(*action, g);
        return true;
    }
    false
}

fn open_job(c: &mut Controller) {
    if let Some(id) = c
        .app
        .notifications
        .history
        .get(c.app.notifications.cursor)
        .and_then(|event| event.job)
        && let Some(index) = c.app.operations.history.iter().position(|job| job.id == id)
    {
        c.app.notifications.open = false;
        c.app.operations.show_history = true;
        c.app.operations.jobs_evidence = true;
        c.app.operations.cursor = index;
        c.app.operations.scroll = 0;
    }
}
