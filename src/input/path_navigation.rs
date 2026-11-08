//! Path input reuses pane-local navigation and captures picker events.
use super::Controller;
use crate::{
    app::Request,
    navigation::{PathPicker, path_segments},
    ui::Geometry,
};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

pub(super) fn open(c: &mut Controller, g: &Geometry) {
    let pane = c.app.active;
    let path = c.app.panes[pane].path.clone();
    let selected = path_segments(&path).len().saturating_sub(1);
    c.last_click = None;
    c.dragging_divider = false;
    c.app.path_pointer = None;
    c.app.path_picker = Some(PathPicker {
        pane,
        path,
        selected,
        scroll: 0,
    });
    let next = crate::ui::geometry(&c.app, g.area);
    reveal(c, &next);
}

pub(super) fn handle(c: &mut Controller, event: &Event, g: &Geometry) -> Option<Vec<Request>> {
    c.app.path_picker.as_ref()?;
    if super::operations::interrupt(event) {
        c.app.path_picker = None;
        return None;
    }
    let state = c.app.path_picker.as_ref().unwrap();
    if stale_or_blocked(c, state) {
        c.app.path_picker = None;
        return Some(Vec::new());
    }
    let requests = match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => key_input(c, *key, g),
        Event::Mouse(mouse) => picker_mouse(c, *mouse, g),
        Event::Resize(width, height) => {
            super::scroll::resize(c, *width, *height);
            c.app.path_pointer = None;
            Vec::new()
        }
        _ => Vec::new(),
    };
    Some(requests)
}

fn stale_or_blocked(c: &Controller, state: &PathPicker) -> bool {
    c.app.panes[state.pane].path != state.path
        || c.app.operations.dialog.is_some()
        || !c.app.operations.conflicts.is_empty()
}

fn key_input(c: &mut Controller, key: crossterm::event::KeyEvent, g: &Geometry) -> Vec<Request> {
    if key.kind == KeyEventKind::Press
        && (key.code == KeyCode::Esc || c.bindings.action(key) == Some(super::Action::Cancel))
    {
        c.app.path_picker = None;
        return Vec::new();
    }
    if !(key.modifiers.is_empty()
        || key.modifiers == KeyModifiers::SHIFT
            && matches!(key.code, KeyCode::Tab | KeyCode::BackTab))
    {
        return Vec::new();
    }
    picker_key(c, key, g)
}

fn picker_key(c: &mut Controller, key: crossterm::event::KeyEvent, g: &Geometry) -> Vec<Request> {
    if key.code == KeyCode::Enter && key.kind == KeyEventKind::Press {
        let state = c.app.path_picker.as_ref().unwrap();
        if state.selected == path_segments(&state.path).len().saturating_sub(1) {
            return navigate(c, state.pane, state.path.clone());
        }
        if let Some(layout) = &g.path_picker {
            let state = c.app.path_picker.as_ref().unwrap();
            if layout
                .controls
                .iter()
                .any(|(_, index)| *index == state.selected)
            {
                let target = path_segments(&state.path)[state.selected].path.clone();
                return navigate(c, state.pane, target);
            }
        }
        return Vec::new();
    }
    if select(c, key.code) {
        reveal(c, g);
    }
    Vec::new()
}

fn select(c: &mut Controller, code: KeyCode) -> bool {
    let state = c.app.path_picker.as_mut().unwrap();
    let count = path_segments(&state.path).len();
    match code {
        KeyCode::Left | KeyCode::Up | KeyCode::BackTab => {
            state.selected = state.selected.saturating_sub(1)
        }
        KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
            state.selected = (state.selected + 1).min(count.saturating_sub(1))
        }
        KeyCode::Home => state.selected = 0,
        KeyCode::End => state.selected = count.saturating_sub(1),
        _ => return false,
    }
    true
}

pub(super) fn reveal(c: &mut Controller, g: &Geometry) {
    let Some(layout) = &g.path_picker else { return };
    let state = c.app.path_picker.as_mut().unwrap();
    let row = layout
        .segment_rows
        .get(state.selected)
        .copied()
        .unwrap_or(0);
    let height = usize::from(layout.shell.body.height).max(1);
    state.scroll = state
        .scroll
        .min(row)
        .max(row.saturating_add(1).saturating_sub(height))
        .min(layout.limit);
}

fn scroll(c: &mut Controller, g: &Geometry, delta: isize) {
    if let Some(layout) = &g.path_picker {
        c.app.path_picker.as_mut().unwrap().scroll =
            layout.scroll.saturating_add_signed(delta).min(layout.limit);
    }
}

fn picker_mouse(
    c: &mut Controller,
    mouse: crossterm::event::MouseEvent,
    g: &Geometry,
) -> Vec<Request> {
    let Some(layout) = &g.path_picker else {
        return Vec::new();
    };
    c.app.path_pointer = Some((mouse.column, mouse.row));
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) if mouse.modifiers.is_empty() => {
            if layout.shell.hit(mouse.column, mouse.row).is_some()
                || layout
                    .shell
                    .close
                    .is_some_and(|r| r.contains((mouse.column, mouse.row).into()))
            {
                c.app.path_picker = None;
            } else if let Some((control, _)) = layout
                .controls
                .iter()
                .find(|(control, _)| control.area.contains((mouse.column, mouse.row).into()))
            {
                return navigate(c, control.pane, control.target.clone().unwrap());
            }
        }
        MouseEventKind::ScrollUp => scroll(c, g, -1),
        MouseEventKind::ScrollDown => scroll(c, g, 1),
        _ => {}
    }
    Vec::new()
}

pub(super) fn header(
    c: &mut Controller,
    event: crossterm::event::MouseEvent,
    g: &Geometry,
) -> Option<Vec<Request>> {
    c.app.path_pointer = Some((event.column, event.row));
    if event.kind != MouseEventKind::Down(MouseButton::Left) || !event.modifiers.is_empty() {
        return None;
    }
    let control = g.path_controls.iter().find(|control| {
        control.area.contains((event.column, event.row).into())
            && c.app.panes[control.pane].path == control.source
    })?;
    c.last_click = None;
    if let Some(target) = &control.target {
        return Some(navigate(c, control.pane, target.clone()));
    }
    c.app.active = control.pane;
    c.app.sidebar = c.app.panes[control.pane].sidebar;
    open(c, g);
    Some(Vec::new())
}

fn navigate(c: &mut Controller, pane: usize, path: std::path::PathBuf) -> Vec<Request> {
    c.app.path_picker = None;
    c.app.path_pointer = None;
    c.app.active = pane;
    c.app.sidebar = c.app.panes[pane].sidebar;
    if path == c.app.panes[pane].path {
        return Vec::new();
    }
    if Some(path.as_path()) == c.app.panes[pane].path.parent() {
        return c.app.parent().into_iter().collect();
    }
    vec![c.app.request_listing(pane, path)]
}
