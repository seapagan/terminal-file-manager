//! Pattern form owns input until cancellation or an explicit measured action.
use super::Controller;
use crate::{
    app::{PatternMode, PatternSelection},
    ui::{Geometry, PatternControl},
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};

pub(super) fn open(controller: &mut Controller) {
    let pane = controller.app.active;
    controller.app.pattern = Some(PatternSelection::new(&controller.app.panes[pane], pane));
    controller.last_click = None;
    controller.dragging_divider = false;
}

pub(super) fn handle(controller: &mut Controller, event: &Event, geometry: &Geometry) -> bool {
    if controller.app.pattern.is_none() {
        return false;
    }
    if controller.app.operations.dialog.is_some() || !controller.app.operations.conflicts.is_empty()
    {
        return dismiss_for_priority(controller, event);
    }
    if interrupt(event) {
        controller.app.pattern = None;
        return false;
    }
    match event {
        Event::Key(key) => key_input(controller, *key, geometry),
        Event::Paste(text) if controller.app.pattern.as_ref().unwrap().focus == 0 => {
            append(controller, text)
        }
        Event::Mouse(mouse) => mouse_input(controller, *mouse, geometry),
        Event::Resize(width, height) => super::scroll::resize(controller, *width, *height),
        _ => {}
    }
    true
}

fn dismiss_for_priority(controller: &mut Controller, event: &Event) -> bool {
    controller.app.pattern = None;
    if let Event::Resize(width, height) = event {
        super::scroll::resize(controller, *width, *height);
    }
    // The event belonged to the pattern layer, never to a newly arrived approval.
    !interrupt(event)
}

fn key_input(controller: &mut Controller, key: KeyEvent, geometry: &Geometry) {
    if ignored_key(key) {
        return;
    }
    if key.code == KeyCode::Esc && key.kind == KeyEventKind::Press {
        controller.app.pattern = None;
        controller.app.status = "Pattern selection cancelled".into();
        return;
    }
    let Some(layout) = &geometry.pattern else {
        return;
    };
    if shortcut(controller, key, layout) || key.modifiers.contains(KeyModifiers::ALT) {
        return;
    }
    route_key(controller, key, layout);
}

fn route_key(controller: &mut Controller, key: KeyEvent, layout: &crate::ui::PatternLayout) {
    match key.code {
        KeyCode::Tab | KeyCode::BackTab if key.kind == KeyEventKind::Press => {
            advance_focus(controller, key, layout)
        }
        KeyCode::Enter if key.kind == KeyEventKind::Press => enter(controller, layout),
        KeyCode::PageUp | KeyCode::PageDown => {
            let state = controller.app.pattern.as_mut().unwrap();
            state.scroll = state
                .scroll
                .min(layout.scroll_limit)
                .saturating_add_signed(if key.code == KeyCode::PageUp { -3 } else { 3 })
                .min(layout.scroll_limit);
        }
        _ => edit(controller, key, layout),
    }
}

fn ignored_key(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Release
        || key.modifiers.intersects(
            KeyModifiers::CONTROL | KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META,
        )
        || (key.modifiers.contains(KeyModifiers::ALT) && !matches!(key.code, KeyCode::Char(_)))
}

fn shortcut(controller: &mut Controller, key: KeyEvent, layout: &crate::ui::PatternLayout) -> bool {
    use crate::app::PatternOperation::*;
    if key.kind != KeyEventKind::Press
        || (controller.app.pattern.as_ref().unwrap().focus == 0
            && !key.modifiers.contains(KeyModifiers::ALT))
    {
        return false;
    }
    let KeyCode::Char(letter) = key.code else {
        return false;
    };
    let target = match letter.to_ascii_lowercase() {
        'w' => PatternControl::Mode(PatternMode::Wildcard),
        'x' => PatternControl::Mode(PatternMode::Regex),
        's' => PatternControl::Case,
        'm' => PatternControl::Apply(Mark),
        'u' => PatternControl::Apply(Unmark),
        'r' => PatternControl::Apply(Replace),
        _ => return false,
    };
    if layout.targets().contains(&target) {
        let move_focus = matches!(target, PatternControl::Apply(_));
        activate(controller, target, move_focus);
    }
    true
}

fn advance_focus(controller: &mut Controller, key: KeyEvent, layout: &crate::ui::PatternLayout) {
    let state = controller.app.pattern.as_mut().unwrap();
    let targets = layout.targets();
    let current = targets
        .iter()
        .position(|target| index(*target) == state.focus)
        .unwrap_or(0);
    let delta = if key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT) {
        -1
    } else {
        1
    };
    if !targets.is_empty() {
        state.focus =
            index(targets[(current as isize + delta).rem_euclid(targets.len() as isize) as usize]);
    }
}

fn enter(controller: &mut Controller, layout: &crate::ui::PatternLayout) {
    let state = controller.app.pattern.as_mut().unwrap();
    if state.focus == 0 {
        if layout
            .targets()
            .contains(&PatternControl::Apply(crate::app::PatternOperation::Mark))
        {
            state.focus = 4;
        }
    } else if let Some(target) = layout
        .targets()
        .into_iter()
        .find(|target| index(*target) == state.focus)
    {
        activate(controller, target, true);
    }
}

fn edit(controller: &mut Controller, key: KeyEvent, layout: &crate::ui::PatternLayout) {
    let state = controller.app.pattern.as_mut().unwrap();
    match key.code {
        KeyCode::Backspace if state.focus == 0 => {
            let mut text = state.expression().to_owned();
            text.pop();
            state.set_expression(text);
        }
        KeyCode::Char(c) if state.focus == 0 => append(controller, &c.to_string()),
        KeyCode::Char(' ') if key.kind == KeyEventKind::Press => {
            if let Some(target @ (PatternControl::Mode(_) | PatternControl::Case)) = layout
                .targets()
                .into_iter()
                .find(|target| index(*target) == state.focus)
            {
                activate(controller, target, true);
            }
        }
        _ => {}
    }
}

fn append(controller: &mut Controller, text: &str) {
    let state = controller.app.pattern.as_mut().unwrap();
    let mut expression = state.expression().to_owned();
    for c in text.chars().filter(|c| !c.is_control()) {
        if expression.len() + c.len_utf8() > 4096 {
            break;
        }
        expression.push(c);
    }
    state.set_expression(expression);
}

fn activate(controller: &mut Controller, target: PatternControl, move_focus: bool) {
    let mut state = controller.app.pattern.take().unwrap();
    if move_focus {
        state.focus = index(target);
    }
    match target {
        PatternControl::Cancel => {
            controller.app.status = "Pattern selection cancelled".into();
            return;
        }
        PatternControl::Expression => {}
        PatternControl::Mode(mode) => state.set_mode(mode),
        PatternControl::Case => state.toggle_case(),
        PatternControl::Apply(operation) => {
            match state.apply(&mut controller.app.panes[state.pane], operation) {
                Ok(preview) => {
                    controller.app.status = format!(
                        "Pattern selection: {} matches; visible marks updated",
                        preview.matches
                    );
                    return;
                }
                Err(error) => controller.app.status = error,
            }
        }
    }
    controller.app.pattern = Some(state);
}

fn index(target: PatternControl) -> usize {
    use crate::app::PatternOperation::*;
    match target {
        PatternControl::Expression => 0,
        PatternControl::Mode(PatternMode::Wildcard) => 1,
        PatternControl::Mode(PatternMode::Regex) => 2,
        PatternControl::Case => 3,
        PatternControl::Apply(Mark) => 4,
        PatternControl::Apply(Unmark) => 5,
        PatternControl::Apply(Replace) => 6,
        PatternControl::Cancel => 7,
    }
}

fn interrupt(event: &Event) -> bool {
    match event {
        Event::Key(key) => {
            key.kind != KeyEventKind::Release
                && key.code == KeyCode::Char('c')
                && key.modifiers == KeyModifiers::CONTROL
        }
        _ => false,
    }
}

fn mouse_input(
    controller: &mut Controller,
    mouse: crossterm::event::MouseEvent,
    geometry: &Geometry,
) {
    let Some(layout) = &geometry.pattern else {
        return;
    };
    match mouse.kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
            if layout.shell.body.contains((mouse.column, mouse.row).into()) =>
        {
            let state = controller.app.pattern.as_mut().unwrap();
            state.scroll = state
                .scroll
                .min(layout.scroll_limit)
                .saturating_add_signed(if mouse.kind == MouseEventKind::ScrollUp {
                    -1
                } else {
                    1
                })
                .min(layout.scroll_limit);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(target) = layout.hit(mouse.column, mouse.row) {
                activate(controller, target, true);
            }
        }
        _ => {}
    }
}
