use super::Action;
use crossterm::event::{KeyCode, KeyModifiers};

pub(super) fn key_name(code: KeyCode, modifiers: KeyModifiers) -> String {
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
        KeyCode::Char(c) if !modifiers.is_empty() => c.to_uppercase().to_string(),
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
    (HumanSizes, "Toggle human sizes"),
    (FuzzyTimestamps, "Toggle fuzzy timestamps"),
    (Permissions, "Cycle permission display"),
    (ColumnSize, "Toggle size column"),
    (ColumnModified, "Toggle date column"),
    (ColumnAttributes, "Toggle attribute column"),
    (OwnerDisplay, "Toggle owner/group names or numeric IDs"),
    (ColumnOwner, "Toggle owner column"),
    (
        MetadataPresentation,
        "Cycle preferred metadata presentation",
    ),
    (Up, "Move cursor up"),
    (Down, "Move cursor down"),
    (ExtendUp, "Extend marked range up"),
    (ExtendDown, "Extend marked range down"),
    (Home, "First entry"),
    (End, "Last entry"),
    (PageUp, "Previous page"),
    (PageDown, "Next page"),
    (Enter, "Enter directory; files never launch"),
    (Parent, "Parent directory"),
    (Back, "Back in pane history"),
    (Forward, "Forward in pane history"),
    (SaveQuit, "Save navigation session and quit"),
    (Quit, "Quit without saving a navigation session"),
    (Help, "Toggle help"),
    (TogglePanes, "Toggle one/two panes"),
    (FocusPane, "Focus other visible pane"),
    (Maximise, "Temporarily maximise active pane / restore split"),
    (SplitLeft, "Move pane divider left"),
    (SplitRight, "Move pane divider right"),
    (ToggleSidebar, "Toggle metadata sidebar"),
    (Metadata, "Toggle metadata popup"),
    (Info, "Toggle preferred metadata presentation"),
    (Count, "Count directory descendants (cancellable)"),
    (Cancel, "Close overlay, cancel count or clear filter"),
    (Mark, "Toggle mark"),
    (MarkNext, "Toggle mark and move down"),
    (SelectAll, "Mark all visible entries"),
    (ClearVisible, "Clear visible marks"),
    (
        Filter,
        "Edit live fuzzy filter; Enter accepts, Esc restores",
    ),
    (Hidden, "Toggle hidden entries"),
    (Ignore, "Cycle Git-ignore Dim/Hide/Off"),
    (Refresh, "Refresh active directory"),
    (Sort, "Cycle sort order"),
    (Reverse, "Reverse sort direction"),
    (Group, "Cycle directory grouping First/Last/Mixed"),
    (Viewer, "Unavailable until M6: viewer"),
    (Editor, "Unavailable until M6: editor"),
    (Copy, "Unavailable until M2: copy"),
    (Move, "Unavailable until M2: move"),
    (Mkdir, "Unavailable until M2: create directory"),
    (Trash, "Unavailable until M2: Trash"),
    (Delete, "Unavailable until M2: permanent delete"),
    (Path, "Unavailable until M5: path entry"),
    (Zoxide, "Reserved for zoxide (M5)"),
    (OppositeZoxide, "Reserved for opposite-pane zoxide (M5)"),
    (ExitDirectory, "Unavailable until M5: shell handoff"),
];
