//! Context Help stores only its own topic and scroll, leaving parent state in place.
use crate::{
    app::App,
    input::Controller,
    operations::{Dialog, Operation},
    ui::Geometry,
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpTopic {
    Pattern,
    ViewOptions,
    ViewProfiles,
    SaveProfile,
    Operation(Operation),
    Conflict,
    Jobs,
    Metadata,
    Exit,
    Incident,
}

#[derive(Clone, Debug)]
pub struct ContextHelp {
    pub topic: HelpTopic,
    pub scroll: u16,
}

impl HelpTopic {
    pub fn current(app: &App) -> Option<Self> {
        if app.help || app.diagnostics {
            return None;
        }
        if app.popup {
            return Some(Self::Metadata);
        }
        Self::menu(app)
            .or_else(|| Self::dialog(app))
            .or_else(|| Self::workspace(app))
    }

    fn menu(app: &App) -> Option<Self> {
        if app.view_menu.is_some() {
            Some(Self::ViewOptions)
        } else if app.profile_menu.is_some() {
            Some(if app.profile_name.is_some() {
                Self::SaveProfile
            } else {
                Self::ViewProfiles
            })
        } else {
            None
        }
    }

    fn dialog(app: &App) -> Option<Self> {
        match &app.operations.dialog {
            Some(Dialog::Operation(dialog)) => Some(Self::Operation(dialog.plan.operation)),
            Some(Dialog::Exit { .. }) => Some(Self::Exit),
            Some(Dialog::IncidentExit { .. }) => Some(Self::Incident),
            None => None,
        }
    }

    fn workspace(app: &App) -> Option<Self> {
        if app.operations.show_history {
            Some(Self::Jobs)
        } else if !app.operations.conflicts.is_empty() {
            Some(Self::Conflict)
        } else if app.operations.waiting_exit.is_some() {
            Some(Self::Exit)
        } else {
            app.pattern.as_ref().map(|_| Self::Pattern)
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::ViewOptions => "View Options Help",
            Self::ViewProfiles => "View Profiles Help",
            Self::SaveProfile => "Save Profile Help",
            Self::Operation(operation) => operation_title(operation),
            _ => workspace_title(self),
        }
    }

    pub fn text(self, app: &App) -> Vec<super::HelpParagraph> {
        super::context_text::text(self, app)
    }
}

fn workspace_title(topic: HelpTopic) -> &'static str {
    match topic {
        HelpTopic::Pattern => "Pattern Help",
        HelpTopic::Conflict => "Conflict Help",
        HelpTopic::Jobs => "Jobs Help",
        HelpTopic::Metadata => "Metadata Help",
        HelpTopic::Exit => "Safe Exit Help",
        HelpTopic::Incident => "Incident Help",
        _ => unreachable!("workspace Help topic"),
    }
}

fn operation_title(operation: Operation) -> &'static str {
    match operation {
        Operation::Copy => "Copy Help",
        Operation::Move => "Move Help",
        Operation::Mkdir => "Create Directory Help",
        Operation::CreateFile => "Create File Help",
        Operation::Rename => "Rename Help",
        Operation::Trash => "Trash Help",
        Operation::Delete => "Permanent Delete Help",
        Operation::Permissions(_) | Operation::ReadOnly(_) => "Attributes Help",
    }
}

pub(in crate::input) fn handle(c: &mut Controller, event: &Event, geometry: &Geometry) -> bool {
    if crate::input::operations::interrupt(event) {
        c.app.context_help = None;
        return false;
    }
    if c.app.context_help.is_some() {
        overlay(c, event, geometry);
        return true;
    }
    let Some(topic) = HelpTopic::current(&c.app) else {
        return false;
    };
    let open = match event {
        Event::Key(key) => opens_with_key(c, *key, topic),
        Event::Mouse(mouse) => {
            mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && geometry
                    .context_control
                    .is_some_and(|area| area.contains((mouse.column, mouse.row).into()))
        }
        _ => false,
    };
    if open {
        c.app.context_help = Some(ContextHelp { topic, scroll: 0 });
    }
    open
}

fn opens_with_key(c: &Controller, key: KeyEvent, topic: HelpTopic) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }
    if key.code == KeyCode::F(1) && key.modifiers.is_empty() {
        return true;
    }
    key.code == KeyCode::Char('?')
        && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
        && c.bindings.action(key) == Some(crate::input::Action::Help)
        && !editing(&c.app, topic)
}

fn editing(app: &App, topic: HelpTopic) -> bool {
    match topic {
        HelpTopic::Pattern => app
            .pattern
            .as_ref()
            .is_some_and(|state| state.focus == crate::ui::PatternControl::Expression),
        HelpTopic::Operation(operation) => {
            !matches!(operation, Operation::Delete | Operation::Trash)
        }
        HelpTopic::Incident | HelpTopic::SaveProfile => true,
        _ => false,
    }
}

fn overlay(c: &mut Controller, event: &Event, geometry: &Geometry) {
    let limit = geometry
        .context_help
        .as_ref()
        .map_or(0, |layout| layout.scroll_limit);
    let page = geometry
        .context_help
        .as_ref()
        .map_or(1, |layout| layout.shell.body.height.max(1));
    if let Event::Resize(width, height) = event {
        let geometry =
            crate::ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, *width, *height));
        let limit = geometry
            .context_help
            .map_or(0, |layout| layout.scroll_limit);
        let state = c.app.context_help.as_mut().expect("context layer");
        state.scroll = state.scroll.min(limit);
        return;
    }
    if matches!(event, Event::Key(key) if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc)
        || matches!(event, Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) && geometry.context_help.as_ref().is_some_and(|layout| layout.shell.hit(mouse.column, mouse.row).is_some()))
    {
        c.app.context_help = None;
        return;
    }
    let delta = scroll_delta(event, geometry, page);
    let state = c.app.context_help.as_mut().expect("context layer");
    state.scroll = state
        .scroll
        .min(limit)
        .saturating_add_signed(delta)
        .min(limit);
}

fn scroll_delta(event: &Event, geometry: &Geometry, page: u16) -> i16 {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release && key.modifiers.is_empty() => {
            key_scroll(key.code, page)
        }
        Event::Mouse(mouse)
            if geometry.context_help.as_ref().is_some_and(|layout| {
                layout.shell.body.contains((mouse.column, mouse.row).into())
            }) =>
        {
            match mouse.kind {
                MouseEventKind::ScrollUp => -1,
                MouseEventKind::ScrollDown => 1,
                _ => 0,
            }
        }
        _ => 0,
    }
}

fn key_scroll(code: KeyCode, page: u16) -> i16 {
    let page = page.min(i16::MAX as u16) as i16;
    match code {
        KeyCode::Up => -1,
        KeyCode::Down => 1,
        KeyCode::PageUp => -page,
        KeyCode::PageDown => page,
        KeyCode::Home => i16::MIN,
        KeyCode::End => i16::MAX,
        _ => 0,
    }
}
