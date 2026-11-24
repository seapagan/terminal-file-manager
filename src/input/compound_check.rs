//! Startup review captures input until choices are committed or declined.
use super::Controller;
use crate::{
    settings::compound_check::{Review, Stage},
    ui::{CheckIntent, Geometry, modal::actions::Group},
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};

impl Controller {
    /// Begin the startup check explicitly, after configuration and startup validation.
    ///
    /// # Errors
    /// Reports malformed version markers; never resets invalid configuration.
    pub fn check_compound_extensions(&mut self, running: &str) -> Result<(), String> {
        if self.app.compound_extension_review.is_some() {
            return Ok(());
        }
        let (review, completed) = self.preferences.check_compound_extensions(running)?;
        self.app.compound_extension_review = review;
        self.preferences_changed |= completed;
        Ok(())
    }

    fn finish_compound_check(&mut self, remove: bool) {
        if let Some(review) = self.app.compound_extension_review.take() {
            review.complete(&mut self.preferences, remove);
            self.preferences_changed = true;
        }
    }
}

pub(super) fn handle(c: &mut Controller, event: &Event, geometry: &Geometry) -> bool {
    if c.app.compound_extension_review.is_none() {
        return false;
    }
    let intent = match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => key_input(c, *key, geometry),
        Event::Mouse(mouse) => mouse_input(c, *mouse, geometry),
        Event::Resize(width, height) => {
            let next =
                crate::ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, *width, *height));
            if let Some(layout) = next.compound_extension_review {
                c.app.compound_extension_review.as_mut().unwrap().scroll = layout.scroll;
            }
            None
        }
        _ => None,
    };
    if let Some(intent) = intent {
        activate(c, intent);
    }
    true
}

fn activate(c: &mut Controller, intent: CheckIntent) {
    let review = c.app.compound_extension_review.as_mut().unwrap();
    let complete = match (review.stage, intent) {
        (Stage::Replacement, CheckIntent::Keep | CheckIntent::Defaults | CheckIntent::Dismiss) => {
            !review.choose_replacement(intent == CheckIntent::Defaults)
        }
        (Stage::Additions, CheckIntent::Keep | CheckIntent::Remove | CheckIntent::Dismiss) => true,
        _ => false,
    };
    if complete {
        c.finish_compound_check(intent == CheckIntent::Remove);
    }
}

fn key_input(c: &mut Controller, key: KeyEvent, g: &Geometry) -> Option<CheckIntent> {
    if key.code == KeyCode::Esc && key.modifiers.is_empty() {
        return Some(CheckIntent::Dismiss);
    }
    if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
        c.finish_compound_check(false);
        c.dispatch(super::Action::Quit, g);
        return None;
    }
    if !(key.modifiers.is_empty()
        || key.modifiers == KeyModifiers::SHIFT && key.code == KeyCode::BackTab)
    {
        return None;
    }
    let layout = g.compound_extension_review.as_ref()?;
    let review = c.app.compound_extension_review.as_mut().unwrap();
    if layout.stage != review.stage {
        return None;
    }
    if scroll_key(review, layout, key.code) {
        return None;
    }
    action_key(review, layout, key.code)
}

fn scroll_key(review: &mut Review, layout: &crate::ui::CheckLayout, code: KeyCode) -> bool {
    let delta = match code {
        KeyCode::Up => -1,
        KeyCode::Down => 1,
        KeyCode::PageUp => -isize::try_from(layout.shell.body.height).unwrap_or(1),
        KeyCode::PageDown => isize::try_from(layout.shell.body.height).unwrap_or(1),
        KeyCode::Home => {
            review.scroll = 0;
            return true;
        }
        KeyCode::End => {
            review.scroll = layout.limit;
            return true;
        }
        _ => 0,
    };
    if delta != 0 {
        review.scroll = layout.scroll.saturating_add_signed(delta).min(layout.limit);
        return true;
    }
    false
}

fn action_key(
    review: &mut Review,
    layout: &crate::ui::CheckLayout,
    code: KeyCode,
) -> Option<CheckIntent> {
    let mut actions = crate::ui::compound_check::actions(review);
    for (index, action) in actions.iter_mut().enumerate() {
        action.enabled &= layout
            .shell
            .actions
            .iter()
            .any(|r| r.index == index && r.action.enabled);
    }
    let mut group = Group {
        actions,
        selected: review.selected,
        cancel: CheckIntent::Dismiss,
    };
    let code = match code {
        KeyCode::BackTab => KeyCode::Left,
        KeyCode::Char(' ') => KeyCode::Enter,
        code => code,
    };
    let intent = group.key(KeyEvent::new(code, KeyModifiers::NONE));
    review.selected = group.selected;
    intent
}

fn mouse_input(
    c: &mut Controller,
    mouse: crossterm::event::MouseEvent,
    g: &Geometry,
) -> Option<CheckIntent> {
    let layout = g.compound_extension_review.as_ref()?;
    let point = (mouse.column, mouse.row).into();
    let review = c.app.compound_extension_review.as_mut().unwrap();
    if layout.stage != review.stage {
        return None;
    }
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left)
            if layout.shell.close.is_some_and(|r| r.contains(point)) =>
        {
            Some(CheckIntent::Dismiss)
        }
        MouseEventKind::Down(MouseButton::Left) => {
            layout.shell.hit(mouse.column, mouse.row).copied()
        }
        MouseEventKind::ScrollUp => {
            review.scroll = layout.scroll.saturating_sub(3);
            None
        }
        MouseEventKind::ScrollDown => {
            review.scroll = (layout.scroll + 3).min(layout.limit);
            None
        }
        _ => None,
    }
}
