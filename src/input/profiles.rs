use super::Controller;
use crate::{settings::validate_profile_name, view::ViewSettings};
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

pub(super) fn open(controller: &mut Controller) {
    controller.app.profile_menu = Some(0);
    controller.app.profile_choice = 0;
    controller.app.profile_names = controller.preferences.profiles.keys().cloned().collect();
    controller.app.profile_name = None;
    controller.app.profile_error = None;
    controller.app.view_menu = None;
    controller.app.popup = false;
    controller.app.close_help();
}

pub(super) fn key(controller: &mut Controller, key: KeyEvent) {
    if key.kind == KeyEventKind::Repeat
        && !matches!(
            key.code,
            KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End
        )
    {
        if controller.app.profile_name.is_some()
            && matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace)
        {
            edit(controller, key);
        }
        return;
    }
    if controller.app.profile_name.is_some() {
        edit(controller, key);
        return;
    }
    menu_key(controller, key.code);
}

fn menu_key(controller: &mut Controller, code: KeyCode) {
    let index = controller
        .app
        .profile_menu
        .as_mut()
        .expect("profile selector");
    match code {
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
    remember_choice(controller);
}

fn remember_choice(controller: &mut Controller) {
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
        controller.app.profile_error = None;
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
    controller.app.status = format!("View profile “{name}” applied").into();
}

fn edit(controller: &mut Controller, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            controller.app.profile_name = None;
            controller.app.profile_error = None;
        }
        KeyCode::Backspace => {
            controller.app.profile_name.as_mut().unwrap().pop();
            controller.app.profile_error = None;
        }
        KeyCode::Char(c)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            let name = controller.app.profile_name.as_mut().unwrap();
            if name.chars().count() < 64 && !c.is_control() {
                name.push(c);
                controller.app.profile_error = None;
            }
        }
        KeyCode::Enter => save(controller),
        _ => {}
    }
}

fn save(controller: &mut Controller) {
    let name = controller.app.profile_name.as_ref().unwrap().clone();
    if let Err(error) = validate_profile_name(&name) {
        controller.app.profile_error = Some(error);
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
    controller.app.profile_error = None;
    controller.app.profile_name = None;
    controller.app.profile_menu = None;
    controller.app.profile_names = controller.preferences.profiles.keys().cloned().collect();
    controller.app.status = format!("View profile “{name}” saved; preferences save on exit").into();
}

pub(super) fn mouse(
    controller: &mut Controller,
    event: MouseEvent,
    shell: &crate::ui::modal::Layout<()>,
) {
    if controller.app.profile_name.is_some() {
        return;
    }
    match event.kind {
        MouseEventKind::ScrollUp => key(controller, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
        MouseEventKind::ScrollDown => {
            key(controller, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
        }
        MouseEventKind::Down(MouseButton::Left)
            if shell.body.contains((event.column, event.row).into()) =>
        {
            let start = controller
                .app
                .profile_menu
                .unwrap_or(0)
                .saturating_sub(usize::from(shell.body.height.saturating_sub(1)));
            let index = start + usize::from(event.row - shell.body.y);
            if index >= controller.app.profile_names.len() + 2 {
                return;
            }
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

#[cfg(test)]
mod notification_tests {
    use super::*;
    use crate::{input::Bindings, notifications::Severity};

    fn controller() -> Controller {
        let mut c = Controller::new(
            crate::app::App::new(".".into()),
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        );
        c.app.notify(Severity::Error, "Retained failure");
        c
    }

    fn assert_retained(c: &Controller) {
        assert_eq!(c.app.notifications.history.len(), 1);
        assert_eq!(c.app.notifications.history[0].text, "Retained failure");
        assert_eq!(c.app.notifications.visible(), [0]);
    }

    #[test]
    fn profile_apply_uses_status_without_notifying() {
        let mut c = controller();
        open(&mut c);
        choose(&mut c);
        assert_retained(&c);
        assert!(c.app.status.contains("applied"));
        assert!(c.app.profile_menu.is_none());
    }

    #[test]
    fn profile_save_and_validation_do_not_notify() {
        for name in ["Saved", ""] {
            let mut c = controller();
            open(&mut c);
            c.app.profile_name = Some(name.into());
            save(&mut c);
            assert_retained(&c);
            assert_eq!(c.app.profile_name.is_none(), !name.is_empty());
            assert_eq!(c.app.profile_error.is_some(), name.is_empty());
        }
    }

    #[test]
    fn profile_cancel_does_not_publish_a_notification() {
        let mut c = controller();
        let status = c.app.status.clone();
        open(&mut c);
        menu_key(&mut c, KeyCode::Esc);
        assert_eq!(c.app.status, status);
        assert_retained(&c);
    }
}
