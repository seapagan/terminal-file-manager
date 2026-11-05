//! The history overlay owns input without changing its underlying pane or Jobs state.
use super::{Action, Controller};
use crate::ui::Geometry;
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};

pub(super) fn handle(c: &mut Controller, event: &Event, g: &Geometry) -> bool {
    if let Event::Key(key) = event
        && key.kind != KeyEventKind::Release
        && let Some(action) = c.bindings.action(*key)
        && dismiss(c, action, g)
    {
        return true;
    }
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
            c.app.notifications.dismiss_toast(toast.event);
        }
        return true;
    }
    false
}

pub(super) fn dismiss(c: &mut Controller, action: Action, g: &Geometry) -> bool {
    match action {
        Action::DismissToast => {
            if let Some(toast) = g.toasts.last() {
                c.app.notifications.dismiss_toast(toast.event);
            }
        }
        Action::DismissToasts => c.app.notifications.dismiss_toasts(),
        _ => return false,
    }
    true
}

pub(super) fn open(c: &mut Controller) {
    c.app.notifications.dismiss_toasts();
    c.app.notifications.open = true;
    c.app.notifications.cursor = c.app.notifications.latest_index().unwrap_or(0);
    c.app.notifications.scroll = 0;
    c.app.notifications.details = false;
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
        history_body(c, mouse, layout, limit);
    }
}

fn history_body(
    c: &mut Controller,
    mouse: &crossterm::event::MouseEvent,
    layout: &crate::ui::HistoryLayout,
    limit: usize,
) {
    match mouse.kind {
        MouseEventKind::ScrollUp => history_key(c, KeyCode::PageUp, limit),
        MouseEventKind::ScrollDown => history_key(c, KeyCode::PageDown, limit),
        MouseEventKind::Down(MouseButton::Left) if !c.app.notifications.details => {
            let index = layout.start + usize::from(mouse.row - layout.shell.body.y);
            if index < c.app.notifications.history.len() {
                c.app.notifications.cursor = index;
            }
        }
        _ => {}
    }
}

fn history_key(c: &mut Controller, key: KeyCode, limit: usize) {
    match key {
        KeyCode::Esc => c.app.notifications.open = false,
        KeyCode::Char('e' | 'E') => {
            let state = &mut c.app.notifications;
            state.details = !state.details;
            state.scroll = 0;
        }
        KeyCode::Enter => open_job(c),
        _ => navigate(&mut c.app.notifications, key, limit),
    }
}

fn navigate(state: &mut crate::notifications::Notifications, key: KeyCode, limit: usize) {
    let previous = state.cursor;
    let last = state.history.len().saturating_sub(1);
    match key {
        KeyCode::Up => state.cursor = state.cursor.saturating_sub(1),
        KeyCode::Down => state.cursor = (state.cursor + 1).min(last),
        KeyCode::Home => state.cursor = 0,
        KeyCode::End => state.cursor = last,
        KeyCode::PageUp | KeyCode::PageDown => {
            let (position, maximum) = if state.details {
                (&mut state.scroll, limit)
            } else {
                (&mut state.cursor, last)
            };
            *position = if key == KeyCode::PageUp {
                position.saturating_sub(8)
            } else {
                position.saturating_add(8).min(maximum)
            };
        }
        _ => {}
    }
    if previous != state.cursor {
        state.scroll = 0;
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
        if let Some(dialog) = c.app.operations.dialog.take() {
            c.app.operations.suspended_dialog = Some(Box::new((dialog, c.app.operations.scroll)));
        }
        c.app.operations.show_history = true;
        c.app.operations.jobs_evidence = true;
        c.app.operations.cursor = index;
        c.app.operations.scroll = 0;
    }
}
