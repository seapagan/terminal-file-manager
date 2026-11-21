//! Choice menus commit through the existing policy actions; keyboard cycling stays direct.
use super::{Action, Controller};
use crate::{app::App, ui::Geometry};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

#[derive(Clone, Debug)]
pub struct PolicyMenu {
    pub pane: usize,
    pub action: Action,
    pub selected: usize,
}

pub(crate) fn choices(action: Action) -> &'static [&'static str] {
    match action {
        Action::Sort => &[
            "Natural",
            "Name",
            "Extension",
            "Size",
            "Modified",
            "Created",
            "Kind",
            "Version",
            "Unsorted",
        ],
        Action::Group => &["First", "Last", "Mixed"],
        Action::Hidden => &["Hide", "Show"],
        Action::Ignore => &["Dim", "Hide", "Off"],
        _ => &[],
    }
}

pub(crate) fn current(app: &App, pane: usize, action: Action) -> usize {
    let pane = &app.panes[pane];
    let value = match action {
        Action::Sort => format!("{:?}", pane.sort.sort),
        Action::Group => format!("{:?}", pane.sort.grouping),
        Action::Ignore => format!("{:?}", pane.ignore),
        Action::Hidden => if pane.show_hidden { "Show" } else { "Hide" }.into(),
        _ => return 0,
    };
    choices(action)
        .iter()
        .position(|label| *label == value)
        .unwrap_or(0)
}

pub(super) fn open_at(
    c: &mut Controller,
    event: crossterm::event::MouseEvent,
    geometry: &Geometry,
) -> bool {
    let Some(control) = geometry
        .policy_controls
        .iter()
        .find(|control| control.area.contains((event.column, event.row).into()))
    else {
        return false;
    };
    open(c, control);
    true
}

pub(super) fn open(c: &mut Controller, control: &crate::ui::PolicyControl) {
    c.app.active = control.pane;
    c.app.sidebar = c.app.panes[control.pane].sidebar;
    c.app.policy_menu = Some(PolicyMenu {
        pane: control.pane,
        action: control.action,
        selected: current(&c.app, control.pane, control.action),
    });
    c.last_click = None;
}

pub(super) fn handle(c: &mut Controller, event: &Event, g: &Geometry) -> bool {
    if c.app.policy_menu.is_none() {
        return false;
    }
    let apply = match event {
        Event::Key(key) => key_input(c, *key, g),
        Event::Mouse(mouse) => mouse_input(c, *mouse, g),
        Event::Resize(width, height) => {
            super::scroll::resize(c, *width, *height);
            false
        }
        _ => false,
    };
    if apply {
        commit(c, g);
    }
    true
}

fn key_input(c: &mut Controller, key: crossterm::event::KeyEvent, g: &Geometry) -> bool {
    if key.kind == KeyEventKind::Release {
        return false;
    }
    if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
        c.app.policy_menu = None;
        c.dispatch(Action::Quit, g);
        return false;
    }
    if !key.modifiers.is_empty() {
        return false;
    }
    let menu = c.app.policy_menu.as_mut().unwrap();
    let last = choices(menu.action).len() - 1;
    match key.code {
        KeyCode::Esc => c.app.policy_menu = None,
        KeyCode::Up => menu.selected = menu.selected.saturating_sub(1),
        KeyCode::Down => menu.selected = (menu.selected + 1).min(last),
        KeyCode::Home => menu.selected = 0,
        KeyCode::End => menu.selected = last,
        KeyCode::Enter | KeyCode::Char(' ') if key.kind == KeyEventKind::Press => return true,
        _ => {}
    }
    false
}

fn mouse_input(c: &mut Controller, mouse: crossterm::event::MouseEvent, g: &Geometry) -> bool {
    let Some(layout) = &g.policy_menu else {
        return false;
    };
    let menu = c.app.policy_menu.as_mut().unwrap();
    let last = choices(menu.action).len() - 1;
    match mouse.kind {
        MouseEventKind::ScrollUp => menu.selected = menu.selected.saturating_sub(1),
        MouseEventKind::ScrollDown => menu.selected = (menu.selected + 1).min(last),
        MouseEventKind::Down(MouseButton::Left)
            if layout.shell.body.contains((mouse.column, mouse.row).into()) =>
        {
            let selected = layout.start + usize::from(mouse.row - layout.shell.body.y);
            if selected <= last {
                menu.selected = selected;
                return true;
            }
        }
        MouseEventKind::Down(MouseButton::Left) => c.app.policy_menu = None,
        _ => {}
    }
    false
}

fn commit(c: &mut Controller, g: &Geometry) {
    let menu = c.app.policy_menu.take().unwrap();
    if g.policy_menu
        .as_ref()
        .is_none_or(|layout| layout.shell.body.height == 0)
    {
        return;
    }
    c.app.active = menu.pane;
    for _ in 0..choices(menu.action).len() {
        if current(&c.app, menu.pane, menu.action) == menu.selected {
            break;
        }
        c.dispatch(menu.action, g);
    }
}
