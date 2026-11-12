mod context;
mod context_text;
pub(super) use context::handle as context_handle;
pub use context::{ContextHelp, HelpTopic, ViewHelpTopic};
pub use context_text::{HelpFragment, HelpParagraph};
mod controller;
pub(super) use controller::handle;
mod metadata;
mod search;
mod sections;
pub(super) use metadata::{description, priority};
pub use search::{HelpHit, HelpPicker, HelpTarget, help_hits};
pub use sections::HelpSection;

/// Effective bindings and their action identity, shared by Help and shortcut hints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelpEntry {
    pub action: super::Action,
    pub keys: String,
    pub description: &'static str,
}

impl HelpEntry {
    /// Keep a familiar alias when still bound, otherwise use the shortest effective label.
    pub fn preferred_key(&self, preferred: &str) -> Option<&str> {
        let keys = || self.keys.split(" / ").filter(|key| !key.is_empty());
        keys()
            .find(|key| *key == preferred)
            .or_else(|| keys().min_by_key(|key| ratatui::text::Line::raw(*key).width()))
    }
}

use super::Action;
use crossterm::event::{KeyCode, KeyModifiers};

pub(super) fn accepts(action: Action) -> bool {
    matches!(
        action,
        Action::Up
            | Action::Down
            | Action::Home
            | Action::End
            | Action::PageUp
            | Action::PageDown
            | Action::Help
            | Action::Cancel
            | Action::Quit
            | Action::SaveQuit
            | Action::Diagnostics
            | Action::DiagnosticsUp
            | Action::DiagnosticsDown
    )
}

pub(crate) fn key_name(code: KeyCode, modifiers: KeyModifiers) -> String {
    let mut modifiers = modifiers;
    let uppercase = matches!(code, KeyCode::Char(c) if c.is_ascii_uppercase());
    if uppercase {
        modifiers.insert(KeyModifiers::SHIFT);
    }
    let mut name = String::new();
    for (flag, text) in [
        (KeyModifiers::CONTROL, "Ctrl+"),
        (KeyModifiers::ALT, "Alt+"),
        (KeyModifiers::SHIFT, "Shift+"),
    ] {
        if modifiers.contains(flag) {
            name.push_str(text);
        }
    }
    name.push_str(&match code {
        KeyCode::Char(' ') => "Space".into(),
        KeyCode::Char(c) if uppercase => c.to_ascii_lowercase().to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::F(n) => format!("F{n}"),
        _ => format!("{code:?}"),
    });
    name
}
