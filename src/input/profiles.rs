use super::Controller;
use crate::{settings::validate_profile_name, view::ViewSettings};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

pub(super) fn open(controller: &mut Controller) {
    controller.app.profile_menu = Some(0);
    controller.app.profile_choice = 0;
    controller.app.profile_names = controller.preferences.profiles.keys().cloned().collect();
    controller.app.profile_name = None;
    controller.app.view_menu = None;
    controller.app.popup = false;
    controller.app.help = false;
}

pub(super) fn key(controller: &mut Controller, key: KeyEvent) {
    if controller.app.profile_name.is_some() {
        edit(controller, key);
        return;
    }
    let index = controller
        .app
        .profile_menu
        .as_mut()
        .expect("profile selector");
    match key.code {
        KeyCode::Up => *index = index.saturating_sub(1),
        KeyCode::Down => *index = (*index + 1).min(controller.app.profile_names.len() + 1),
        KeyCode::Home => *index = 0,
        KeyCode::End => *index = controller.app.profile_names.len() + 1,
        KeyCode::Char(' ') if *index < controller.app.profile_names.len() => {
            controller.app.profile_choice = *index;
            *index = controller.app.profile_names.len();
            return;
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            choose(controller);
            return;
        }
        KeyCode::Esc => controller.app.profile_menu = None,
        _ => {}
    }
    if let Some(index) = controller.app.profile_menu
        && index < controller.app.profile_names.len()
    {
        controller.app.profile_choice = index;
    }
}

fn choose(controller: &mut Controller) {
    let index = controller.app.profile_menu.expect("profile selector");
    let count = controller.app.profile_names.len();
    if index == count + 1 {
        controller.app.profile_name = Some(
            controller.app.panes[controller.app.active]
                .profile
                .clone()
                .unwrap_or_default(),
        );
        return;
    }
    let choice = if index < count {
        index
    } else {
        controller.app.profile_choice
    };
    let name = controller.app.profile_names[choice].clone();
    let view = controller.preferences.profiles[&name].clone();
    let active = controller.app.active;
    for id in 0..2 {
        if id == active || index == count {
            view.apply(&mut controller.app.panes[id]);
            controller.app.panes[id].profile = Some(name.clone());
            controller.app.panes[id].profile_modified = false;
        }
    }
    controller.app.sidebar = controller.app.panes[active].sidebar;
    controller.preferences.capture_view(&view);
    controller.preferences_changed = true;
    controller.app.profile_menu = None;
}

fn edit(controller: &mut Controller, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => controller.app.profile_name = None,
        KeyCode::Backspace => {
            controller.app.profile_name.as_mut().unwrap().pop();
        }
        KeyCode::Char(c)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            let name = controller.app.profile_name.as_mut().unwrap();
            if name.chars().count() < 64 && !c.is_control() {
                name.push(c);
            }
        }
        KeyCode::Enter => save(controller),
        _ => {}
    }
}

fn save(controller: &mut Controller) {
    let name = controller.app.profile_name.as_ref().unwrap().clone();
    if let Err(error) = validate_profile_name(&name) {
        controller.app.status = error;
        return;
    }
    let pane = &mut controller.app.panes[controller.app.active];
    controller
        .preferences
        .profiles
        .insert(name.clone(), ViewSettings::capture(pane));
    pane.profile = Some(name.clone());
    pane.profile_modified = false;
    let saved = &controller.preferences.profiles[&name];
    for pane in &mut controller.app.panes {
        if pane.profile.as_deref() == Some(name.as_str()) {
            pane.profile_modified = ViewSettings::capture(pane) != *saved;
        }
    }
    controller.preferences_changed = true;
    controller.app.profile_name = None;
    controller.app.profile_menu = None;
    controller.app.profile_names = controller.preferences.profiles.keys().cloned().collect();
}

pub(super) fn mouse(controller: &mut Controller, event: MouseEvent, area: ratatui::layout::Rect) {
    if controller.app.profile_name.is_some() {
        return;
    }
    match event.kind {
        MouseEventKind::ScrollUp => key(controller, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
        MouseEventKind::ScrollDown => {
            key(controller, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
        }
        MouseEventKind::Down(MouseButton::Left)
            if area.contains((event.column, event.row).into()) =>
        {
            let Some(index) =
                crate::ui::profile_hit(&controller.app, area, (event.column, event.row).into())
            else {
                return;
            };
            controller.app.profile_menu = Some(index);
            if index < controller.app.profile_names.len() {
                controller.app.profile_choice = index;
                return;
            }
            choose(controller);
        }
        _ => {}
    }
}
