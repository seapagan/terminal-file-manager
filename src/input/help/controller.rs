use super::{HelpTarget, help_hits};
use crate::{input::Controller, ui::Geometry};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};

pub(in crate::input) fn handle(c: &mut Controller, event: &Event, geometry: &Geometry) -> bool {
    if !c.app.help {
        return false;
    }
    if matches!(event, Event::Key(key) if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL)
    {
        return false;
    }
    if let Event::Resize(width, height) = event {
        crate::input::scroll::resize(c, *width, *height);
        return true;
    }
    if c.app.help_picker.is_some() {
        picker(c, event, geometry);
        return true;
    }
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc => {
            c.app.help = false;
            c.app.help_landing = None;
            c.app.overlay_scroll = 0;
            true
        }
        Event::Key(key)
            if key.kind == KeyEventKind::Press
                && key.code == KeyCode::Char('/')
                && key.modifiers.is_empty() =>
        {
            c.app.help_picker = Some(Default::default());
            true
        }
        Event::Mouse(mouse)
            if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && geometry
                    .help
                    .as_ref()
                    .and_then(|layout| layout.search)
                    .is_some_and(|area| area.contains((mouse.column, mouse.row).into())) =>
        {
            c.app.help_picker = Some(Default::default());
            true
        }
        _ => false,
    }
}

fn picker(c: &mut Controller, event: &Event, geometry: &Geometry) {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => picker_key(c, key, geometry),
        Event::Paste(text) => append(c, text),
        Event::Mouse(mouse) => mouse_picker(c, mouse, geometry),
        _ => {}
    }
}

fn picker_key(c: &mut Controller, key: &KeyEvent, geometry: &Geometry) {
    match key.code {
        KeyCode::Esc if key.kind == KeyEventKind::Press => c.app.help_picker = None,
        KeyCode::Enter if key.kind == KeyEventKind::Press => {
            let state = c.app.help_picker.as_ref().expect("picker");
            if let Some(hit) = help_hits(&c.app.help_rows, &state.query).get(state.selected) {
                jump(c, hit.target, geometry);
            }
        }
        KeyCode::Backspace => {
            let state = c.app.help_picker.as_mut().expect("picker");
            state.query.pop();
            state.selected = 0;
        }
        KeyCode::Char(ch)
            if !key.modifiers.intersects(
                KeyModifiers::CONTROL
                    | KeyModifiers::ALT
                    | KeyModifiers::SUPER
                    | KeyModifiers::META
                    | KeyModifiers::HYPER,
            ) =>
        {
            append(c, &ch.to_string())
        }
        code => picker_movement(c, code),
    }
}

fn picker_movement(c: &mut Controller, code: KeyCode) {
    match code {
        KeyCode::Up => select(c, -1),
        KeyCode::Down => select(c, 1),
        KeyCode::PageUp => select(c, -7),
        KeyCode::PageDown => select(c, 7),
        _ => {}
    }
}

fn mouse_picker(c: &mut Controller, mouse: &crossterm::event::MouseEvent, geometry: &Geometry) {
    let Some(layout) = geometry
        .help
        .as_ref()
        .and_then(|layout| layout.picker.as_ref())
    else {
        return;
    };
    let inside = layout.shell.body.contains((mouse.column, mouse.row).into());
    match mouse.kind {
        MouseEventKind::ScrollDown if inside => select(c, 1),
        MouseEventKind::ScrollUp if inside => select(c, -1),
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some((_, index)) = layout
                .controls
                .iter()
                .find(|(area, _)| area.contains((mouse.column, mouse.row).into()))
            {
                jump(c, layout.hits[*index].target, geometry);
            }
        }
        _ => {}
    }
}

fn append(c: &mut Controller, text: &str) {
    let state = c.app.help_picker.as_mut().expect("picker");
    for ch in text.chars().filter(|ch| !ch.is_control()) {
        if state.query.len() + ch.len_utf8() > 4096 {
            break;
        }
        state.query.push(ch);
    }
    state.selected = 0;
}

fn select(c: &mut Controller, delta: isize) {
    let state = c.app.help_picker.as_mut().expect("picker");
    let count = help_hits(&c.app.help_rows, &state.query).len();
    state.selected = state
        .selected
        .saturating_add_signed(delta)
        .min(count.saturating_sub(1));
}

fn jump(c: &mut Controller, target: HelpTarget, geometry: &Geometry) {
    let Some(layout) = &geometry.help else {
        return;
    };
    let Some((_, row)) = layout
        .anchors
        .iter()
        .find(|(candidate, _)| *candidate == target)
    else {
        return;
    };
    c.app.overlay_scroll = (*row).min(layout.scroll_limit);
    c.app.help_landing = Some((target, c.app.now + std::time::Duration::from_secs(2)));
    c.app.help_picker = None;
}
