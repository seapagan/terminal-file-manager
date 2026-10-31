mod context;
mod context_text;
pub(super) use context::handle as context_handle;
pub use context::{ContextHelp, HelpTopic};
pub use context_text::{HelpFragment, HelpParagraph};
mod controller;
pub(super) use controller::handle;
mod search;
mod sections;
pub use search::{HelpHit, HelpPicker, HelpTarget, help_hits};
pub use sections::HelpSection;

/// Effective bindings and their action identity, shared by Help and shortcut hints.
#[derive(Clone, Debug)]
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

pub(super) fn key_name(code: KeyCode, modifiers: KeyModifiers) -> String {
    let mut modifiers = modifiers;
    let uppercase = matches!(code, KeyCode::Char(c) if c.is_uppercase());
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
        KeyCode::Char(c) if uppercase => c.to_lowercase().collect(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::F(n) => format!("F{n}"),
        _ => format!("{code:?}"),
    });
    name
}

pub(super) fn description(action: Action) -> &'static str {
    DESCRIPTIONS
        .iter()
        .find_map(|(candidate, text)| (*candidate == action).then_some(*text))
        .expect("every action has help")
}

use Action::*;
const DESCRIPTIONS: &[(Action, &str)] = &[
    (ViewOptions, "View options"),
    (ViewProfiles, "Select or save a named view profile"),
    (HumanSizes, "Show rounded or exact sizes"),
    (FuzzyTimestamps, "Show relative or exact dates"),
    (Permissions, "Cycle permission display"),
    (ColumnSize, "Toggle size column"),
    (ColumnModified, "Toggle date column"),
    (ColumnAttributes, "Toggle attribute column"),
    (OwnerDisplay, "Toggle owner/group names or numeric IDs"),
    (ColumnOwner, "Toggle owner column"),
    (MetadataPresentation, "Choose how to show file information"),
    (Up, "Move cursor up"),
    (Down, "Move cursor down"),
    (ExtendUp, "Extend marked range up"),
    (ExtendDown, "Extend marked range down"),
    (Home, "First entry"),
    (End, "Last entry"),
    (PageUp, "Previous page"),
    (PageDown, "Next page"),
    (Enter, "Open a directory; files are not launched"),
    (Parent, "Parent directory"),
    (Back, "Back in pane history"),
    (Forward, "Forward in pane history"),
    (SaveQuit, "Save pane locations and quit"),
    (Quit, "Quit without saving pane locations"),
    (Help, "Help; F1 explains an open dialog"),
    (Diagnostics, "Toggle read-only diagnostics"),
    (DiagnosticsUp, "Scroll diagnostics up"),
    (DiagnosticsDown, "Scroll diagnostics down"),
    (TogglePanes, "Toggle one/two panes"),
    (FocusPane, "Switch to the other pane"),
    (Maximise, "Maximise this pane or restore the split"),
    (SplitLeft, "Move pane divider left"),
    (SplitRight, "Move pane divider right"),
    (
        ToggleSidebar,
        "Show file information sidebar (wide single pane)",
    ),
    (Metadata, "Show or hide file information"),
    (Info, "Show file information in your chosen view"),
    (Count, "Count folder contents; cancel to stop"),
    (Cancel, "Close overlay, cancel count or clear filter"),
    (Mark, "Toggle mark"),
    (MarkNext, "Toggle mark and move down"),
    (SelectAll, "Mark all visible entries"),
    (ClearVisible, "Clear visible marks"),
    (
        PatternSelection,
        "Mark, unmark or replace visible marks (wildcard or regex)",
    ),
    (
        Filter,
        "Filter names; Enter keeps it, Esc restores the old filter",
    ),
    (
        Hidden,
        "Toggle hidden entries (h beside the type indicator)",
    ),
    (Ignore, "Cycle Git-ignore Dim/Hide/Off"),
    (Refresh, "Refresh active directory"),
    (Sort, "Cycle sort order"),
    (Reverse, "Reverse sort direction"),
    (Group, "Cycle directory grouping First/Last/Mixed"),
    (Viewer, "File viewer (not available yet)"),
    (Editor, "File editor (not available yet)"),
    (Copy, "Copy the visible selection"),
    (Move, "Move the visible selection"),
    (
        CopyQuick,
        "With two panes, copy without the selection prompt",
    ),
    (
        MoveQuick,
        "With two panes, move without the selection prompt",
    ),
    (Mkdir, "Create directory"),
    (CreateFile, "Create empty file"),
    (Trash, "Preview and move selection to Trash"),
    (Delete, "Preview and confirm permanent deletion"),
    (Rename, "Rename one visible entry"),
    (Attributes, "Change permissions or read-only settings"),
    (Jobs, "View jobs, cancel work or retry unfinished items"),
    (Path, "Type a path (not available yet)"),
    (Zoxide, "Jump with zoxide (not available yet)"),
    (
        OppositeZoxide,
        "Open a zoxide location in the other pane (not available yet)",
    ),
    (
        ExitDirectory,
        "Return to this folder in your shell (not available yet)",
    ),
];
