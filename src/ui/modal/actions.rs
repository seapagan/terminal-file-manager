//! Safe standard action groups. Returned intents still require domain authorization.
use crate::color::{Role, Theme};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    text::{Line, Span},
    widgets::Paragraph,
};

/// An action's label, typed intent and deliberate shortcut.
#[derive(Clone, Debug)]
pub struct Action<T> {
    /// Visible label, including the shortcut when needed.
    pub label: String,
    /// Stable label budget in display cells, excluding brackets.
    pub min_width: u16,
    /// Optional separately styled trailing toggle state.
    pub state: Option<(&'static str, Role)>,
    /// Intent interpreted and validated by the domain controller.
    pub intent: T,
    /// Exact shortcut. Destructive actions must supply an explicit key.
    pub key: KeyCode,
    /// False actions cannot activate through keyboard or hit testing.
    pub enabled: bool,
    /// Semantic presentation role.
    pub role: Role,
    /// Whether focus + Enter may activate this action.
    pub enter: bool,
}

impl<T> Action<T> {
    /// A caller-authorized custom shortcut; Enter requires explicit opt-in.
    pub fn custom(label: impl Into<String>, intent: T, key: KeyCode) -> Self {
        Self {
            label: label.into(),
            min_width: 0,
            state: None,
            intent,
            key,
            enabled: true,
            role: Role::Key,
            enter: false,
        }
    }
}

/// Standard modal response. It never submits a filesystem operation by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Response {
    /// Caller may validate and accept.
    Accept,
    /// Dismiss without approval.
    Cancel,
}

/// Behavioural presets with safe initial selection and Escape response.
#[derive(Clone, Copy, Debug)]
pub enum Preset {
    /// Informational acknowledgement.
    Ok,
    /// Editable form; Cancel is initially selected.
    OkCancel,
    /// Binary question; No is initially selected.
    YesNo,
    /// Routine confirmation; Cancel is initially selected.
    ConfirmCancel,
    /// Deliberate shortcut required, even when focused; Enter cannot approve.
    Destructive { label: &'static str, key: KeyCode },
}

/// Focus and activation policy for standard or typed custom actions.
pub struct Group<T> {
    /// Actions in visible order.
    pub actions: Vec<Action<T>>,
    /// Initially safe action, retained by the consumer between frames.
    pub selected: usize,
    /// Escape intent.
    pub cancel: T,
}

impl Preset {
    /// Construct an immediately usable group. Domain readiness remains separate.
    pub fn group(self) -> Group<Response> {
        let (label, key, enter) = match self {
            Self::Ok | Self::OkCancel => ("OK", KeyCode::Enter, true),
            Self::YesNo => ("Yes", KeyCode::Char('y'), true),
            Self::ConfirmCancel => ("Confirm", KeyCode::Enter, true),
            Self::Destructive { label, key } => (label, key, false),
        };
        let mut accept = Action::custom(label, Response::Accept, key);
        accept.enter = enter;
        if !enter {
            accept.role = Role::Error;
        }
        let mut actions = vec![accept];
        if !matches!(self, Self::Ok) {
            let mut cancel = Action::custom(
                if matches!(self, Self::YesNo) {
                    "No"
                } else {
                    "Cancel"
                },
                Response::Cancel,
                KeyCode::Esc,
            );
            cancel.enter = true;
            actions.push(cancel);
        }
        Group {
            selected: actions.len() - 1,
            actions,
            cancel: Response::Cancel,
        }
    }
}

impl<T: Clone> Group<T> {
    /// Consume Press events only. Enter cannot bypass disabled or explicit-key actions.
    pub fn key(&mut self, key: KeyEvent) -> Option<T> {
        if key.kind != KeyEventKind::Press
            || key.modifiers.intersects(
                KeyModifiers::CONTROL
                    | KeyModifiers::ALT
                    | KeyModifiers::SUPER
                    | KeyModifiers::HYPER
                    | KeyModifiers::META,
            )
        {
            return None;
        }
        match key.code {
            KeyCode::Esc => Some(self.cancel.clone()),
            KeyCode::Tab | KeyCode::Right => {
                self.advance(1);
                None
            }
            KeyCode::Left => {
                self.advance(-1);
                None
            }
            KeyCode::Enter => self
                .actions
                .get(self.selected)
                .filter(|action| action.enabled && action.enter)
                .map(|a| a.intent.clone()),
            code => self
                .actions
                .iter()
                .find(|a| a.enabled && a.key == code)
                .map(|a| a.intent.clone()),
        }
    }

    fn advance(&mut self, delta: isize) {
        if self.actions.is_empty() {
            return;
        }
        for _ in 0..self.actions.len() {
            self.selected =
                (self.selected as isize + delta).rem_euclid(self.actions.len() as isize) as usize;
            if self.actions[self.selected].enabled {
                break;
            }
        }
    }
}

/// Render exactly the measured rectangles. A disabled label is dimmed, never hidden.
pub fn render<T>(
    frame: &mut Frame,
    layout: &super::Layout<T>,
    theme: &Theme,
    selected: Option<usize>,
) {
    for region in &layout.actions {
        let role = if !region.action.enabled {
            Role::Disabled
        } else if selected == Some(region.index) {
            Role::Cursor
        } else {
            region.action.role
        };
        let padding = " ".repeat(usize::from(
            region
                .area
                .width
                .saturating_sub(2 + super::width(&region.action.label)),
        ));
        let mut spans = vec![Span::raw("["), Span::raw(region.action.label.as_str())];
        if let Some((state, state_role)) = region.action.state {
            spans[1] = Span::raw(
                region
                    .action
                    .label
                    .strip_suffix(state)
                    .unwrap_or(region.action.label.as_str()),
            );
            spans.push(Span::styled(
                state,
                theme
                    .style(state_role)
                    .fg(theme
                        .style(state_role)
                        .fg
                        .unwrap_or(ratatui::style::Color::Reset))
                    .remove_modifier(ratatui::style::Modifier::BOLD),
            ));
        }
        spans.push(Span::raw(format!("{padding}]")));
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(theme.style(role)),
            region.area,
        );
    }
}
