use super::Action;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{BTreeMap, HashMap};

/// Normalized chords; user overrides replace defaults, conflicting aliases fail.
pub struct Bindings {
    keys: HashMap<(KeyCode, KeyModifiers), Action>,
}

impl Bindings {
    /// Aligned help rows come from the effective keymap, including overrides.
    pub fn help_rows(&self) -> Vec<(String, String)> {
        let mut grouped = BTreeMap::<Action, Vec<String>>::new();
        for ((code, modifiers), action) in &self.keys {
            grouped
                .entry(*action)
                .or_default()
                .push(super::help::key_name(*code, *modifiers));
        }
        grouped
            .into_iter()
            .map(|(action, mut keys)| {
                keys.sort();
                (keys.join(" / "), super::help::description(action).into())
            })
            .collect()
    }
    /// Build and validate configurable bindings.
    ///
    /// # Errors
    /// Rejects unknown keys or two differently spelled overrides for the same chord.
    pub fn new(overrides: &BTreeMap<String, Action>) -> Result<Self, String> {
        let mut keys = HashMap::new();
        for (key, action) in DEFAULT_BINDINGS {
            keys.insert(chord(key)?, *action);
        }
        let mut assigned = HashMap::new();
        for (key, action) in overrides {
            let parsed = chord(key)?;
            if assigned.insert(parsed, key).is_some() {
                return Err(format!("conflicting keybinding: {key}"));
            }
            keys.insert(parsed, *action);
        }
        Ok(Self { keys })
    }

    /// Crossterm Shift on uppercase printable keys is normalized consistently.
    pub fn action(&self, event: KeyEvent) -> Option<Action> {
        if event.code == KeyCode::BackTab {
            return self
                .keys
                .get(&(KeyCode::Tab, event.modifiers | KeyModifiers::SHIFT))
                .copied();
        }
        if let Some(action) = self.keys.get(&(event.code, event.modifiers)) {
            return Some(*action);
        }
        let mut modifiers = event.modifiers;
        if matches!(event.code, KeyCode::Char(_)) {
            modifiers.remove(KeyModifiers::SHIFT);
        }
        self.keys.get(&(event.code, modifiers)).copied()
    }
}

/// Parse documented chords without requiring terminal-specific escape encodings.
///
/// # Errors
/// Returns an actionable unknown-key or modifier error.
pub fn chord(text: &str) -> Result<(KeyCode, KeyModifiers), String> {
    let mut parts: Vec<_> = text.split('+').collect();
    let key = parts.pop().unwrap_or("");
    let mut modifiers = KeyModifiers::NONE;
    for modifier in parts {
        modifiers |= match modifier.to_ascii_lowercase().as_str() {
            "ctrl" => KeyModifiers::CONTROL,
            "alt" => KeyModifiers::ALT,
            "shift" => KeyModifiers::SHIFT,
            _ => return Err(format!("unknown modifier in keybinding: {text}")),
        };
    }
    let code = named_key(key)
        .or_else(|| {
            let mut chars = key.chars();
            let c = chars.next()?;
            chars.next().is_none().then_some(KeyCode::Char(c))
        })
        .ok_or_else(|| format!("unknown keybinding: {text}"))?;
    Ok((code, modifiers))
}

fn named_key(key: &str) -> Option<KeyCode> {
    match key.to_ascii_lowercase().as_str() {
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" => Some(KeyCode::PageUp),
        "pagedown" => Some(KeyCode::PageDown),
        "enter" => Some(KeyCode::Enter),
        "backspace" => Some(KeyCode::Backspace),
        "tab" => Some(KeyCode::Tab),
        "space" => Some(KeyCode::Char(' ')),
        "insert" => Some(KeyCode::Insert),
        "esc" => Some(KeyCode::Esc),
        _ => key
            .strip_prefix('F')
            .and_then(|number| number.parse::<u8>().ok())
            .filter(|n| (1..=24).contains(n))
            .map(KeyCode::F),
    }
}

use Action::*;
const DEFAULT_BINDINGS: &[(&str, Action)] = &[
    ("Up", Up),
    ("k", Up),
    ("Down", Down),
    ("j", Down),
    ("Shift+Up", ExtendUp),
    ("Shift+Down", ExtendDown),
    ("Home", Home),
    ("End", End),
    ("PageUp", PageUp),
    ("PageDown", PageDown),
    ("Enter", Enter),
    ("Backspace", Parent),
    ("Alt+Up", Parent),
    ("Alt+Left", Back),
    ("Alt+Right", Forward),
    ("q", Quit),
    ("F10", Quit),
    ("Ctrl+c", Quit),
    ("?", Help),
    ("Alt+p", TogglePanes),
    ("Alt+m", Maximise),
    ("Tab", FocusPane),
    ("Shift+Tab", FocusPane),
    ("Ctrl+Left", SplitLeft),
    ("Ctrl+Right", SplitRight),
    ("Alt+i", ToggleSidebar),
    ("Alt+Enter", Metadata),
    ("F2", Info),
    ("V", ViewOptions),
    ("Alt+v", ViewProfiles),
    ("c", Count),
    ("Esc", Cancel),
    ("Space", Mark),
    ("Insert", MarkNext),
    ("Ctrl+a", SelectAll),
    ("Ctrl+u", ClearVisible),
    ("/", Filter),
    ("h", Hidden),
    ("i", Ignore),
    ("r", Refresh),
    ("s", Sort),
    ("v", Reverse),
    ("g", Group),
    ("F3", Viewer),
    ("F4", Editor),
    ("F5", Copy),
    ("F6", Move),
    ("F7", Mkdir),
    ("F8", Trash),
    ("Shift+F8", Delete),
    ("Ctrl+l", Path),
    ("z", Zoxide),
    ("Z", OppositeZoxide),
    ("Q", SaveQuit),
];
