//! One owner for command descriptions, sections and display priority.
use super::{Action, HelpSection};

pub(in crate::input) fn description(action: Action) -> &'static str {
    metadata(action).1
}

pub(in crate::input) fn priority(action: Action) -> usize {
    COMMANDS
        .iter()
        .position(|(candidate, _, _)| *candidate == action)
        .expect("every action has a command row")
}

impl Action {
    pub fn help_section(self) -> HelpSection {
        metadata(self).0
    }
}

// The same rows produce the table and match: a new Action without metadata is a compile error.
macro_rules! define_commands {
    ($(($action:path, $section:expr, $description:literal $(,)?)),* $(,)?) => {
        const COMMANDS: &[(Action, HelpSection, &str)] = &[
            $(($action, $section, $description)),*
        ];

        fn metadata(action: Action) -> (HelpSection, &'static str) {
            match action {
                $($action => ($section, $description)),*
            }
        }
    };
}

// Row position is display priority; bindings remain in the effective keymap.
define_commands! {
    (Action::Up, HelpSection::Navigation, "Move cursor up"),
    (Action::Down, HelpSection::Navigation, "Move cursor down"),
    (
        Action::Enter,
        HelpSection::Navigation,
        "Open a directory or focused ..; files are not launched",
    ),
    (Action::Parent, HelpSection::Navigation, "Parent directory"),
    (
        Action::Back,
        HelpSection::Navigation,
        "Back in pane history",
    ),
    (
        Action::Forward,
        HelpSection::Navigation,
        "Forward in pane history",
    ),
    (
        Action::Refresh,
        HelpSection::Navigation,
        "Refresh active directory",
    ),
    (Action::PageDown, HelpSection::Navigation, "Next page"),
    (Action::PageUp, HelpSection::Navigation, "Previous page"),
    (Action::Home, HelpSection::Navigation, "First row (.. when available)"),
    (Action::End, HelpSection::Navigation, "Last entry"),
    (
        Action::Path,
        HelpSection::Navigation,
        "Type a path (not available yet)",
    ),
    (
        Action::Zoxide,
        HelpSection::Navigation,
        "Jump with zoxide (not available yet)",
    ),
    (
        Action::OppositeZoxide,
        HelpSection::Navigation,
        "Open a zoxide location in the other pane (not available yet)",
    ),
    (
        Action::ExitDirectory,
        HelpSection::Navigation,
        "Return to this folder in your shell (not available yet)",
    ),
    (Action::Mark, HelpSection::Selection, "Toggle mark"),
    (
        Action::MarkNext,
        HelpSection::Selection,
        "Toggle mark and move down",
    ),
    (
        Action::ExtendUp,
        HelpSection::Selection,
        "Extend marked range up",
    ),
    (
        Action::ExtendDown,
        HelpSection::Selection,
        "Extend marked range down",
    ),
    (
        Action::SelectAll,
        HelpSection::Selection,
        "Mark all visible entries",
    ),
    (
        Action::ClearVisible,
        HelpSection::Selection,
        "Clear visible marks",
    ),
    (
        Action::PatternSelection,
        HelpSection::Selection,
        "Mark, unmark or replace visible marks (wildcard or regex)",
    ),
    (
        Action::Copy,
        HelpSection::Operations,
        "Copy the visible selection",
    ),
    (
        Action::Move,
        HelpSection::Operations,
        "Move the visible selection",
    ),
    (
        Action::Rename,
        HelpSection::Operations,
        "Rename one visible entry",
    ),
    (Action::Mkdir, HelpSection::Operations, "Create directory"),
    (
        Action::CreateFile,
        HelpSection::Operations,
        "Create empty file",
    ),
    (
        Action::Trash,
        HelpSection::Operations,
        "Preview and move selection to Trash",
    ),
    (
        Action::CopyQuick,
        HelpSection::Operations,
        "With two panes, copy without the selection prompt",
    ),
    (
        Action::MoveQuick,
        HelpSection::Operations,
        "With two panes, move without the selection prompt",
    ),
    (
        Action::Attributes,
        HelpSection::Operations,
        "Change permissions or read-only settings",
    ),
    (
        Action::Delete,
        HelpSection::Operations,
        "Preview and confirm permanent deletion",
    ),
    (
        Action::Jobs,
        HelpSection::Jobs,
        "View jobs, cancel work or retry unfinished items",
    ),
    (
        Action::FocusPane,
        HelpSection::Panes,
        "Switch to the other pane",
    ),
    (
        Action::TogglePanes,
        HelpSection::Panes,
        "Toggle one/two panes",
    ),
    (
        Action::Maximise,
        HelpSection::Panes,
        "Maximise this pane or restore the split",
    ),
    (
        Action::SplitLeft,
        HelpSection::Panes,
        "Move pane divider left",
    ),
    (
        Action::SplitRight,
        HelpSection::Panes,
        "Move pane divider right",
    ),
    (
        Action::Filter,
        HelpSection::Views,
        "Filter names; Enter keeps it, Esc restores the old filter",
    ),
    (
        Action::Hidden,
        HelpSection::Views,
        "Toggle hidden entries (h beside the type indicator)",
    ),
    (Action::Sort, HelpSection::Views, "Cycle sort order"),
    (
        Action::Reverse,
        HelpSection::Views,
        "Reverse sort direction",
    ),
    (
        Action::Group,
        HelpSection::Views,
        "Cycle directory grouping First/Last/Mixed",
    ),
    (
        Action::Ignore,
        HelpSection::Views,
        "Cycle Git-ignore Dim/Hide/Off",
    ),
    (
        Action::Info,
        HelpSection::Views,
        "Show file information in your chosen view",
    ),
    (
        Action::Metadata,
        HelpSection::Views,
        "Show or hide file information",
    ),
    (
        Action::ToggleSidebar,
        HelpSection::Views,
        "Show file information sidebar (wide single pane)",
    ),
    (
        Action::Count,
        HelpSection::Views,
        "Count folder contents; cancel to stop",
    ),
    (Action::ViewOptions, HelpSection::Views, "View options"),
    (
        Action::ViewProfiles,
        HelpSection::Views,
        "Select or save a named view profile",
    ),
    (
        Action::MetadataPresentation,
        HelpSection::Views,
        "Choose how to show file information",
    ),
    (
        Action::HumanSizes,
        HelpSection::Views,
        "Show rounded or exact sizes",
    ),
    (
        Action::FuzzyTimestamps,
        HelpSection::Views,
        "Show relative or exact dates",
    ),
    (
        Action::Permissions,
        HelpSection::Views,
        "Cycle permission display",
    ),
    (
        Action::OwnerDisplay,
        HelpSection::Views,
        "Toggle owner/group names or numeric IDs",
    ),
    (Action::ColumnSize, HelpSection::Views, "Toggle size column"),
    (
        Action::ColumnModified,
        HelpSection::Views,
        "Toggle date column",
    ),
    (
        Action::ColumnAttributes,
        HelpSection::Views,
        "Toggle attribute column",
    ),
    (
        Action::ColumnOwner,
        HelpSection::Views,
        "Toggle owner column",
    ),
    (
        Action::Viewer,
        HelpSection::Views,
        "File viewer (not available yet)",
    ),
    (
        Action::Editor,
        HelpSection::Views,
        "File editor (not available yet)",
    ),
    (Action::DismissToast, HelpSection::Jobs, "Dismiss oldest visible toast; available in all interfaces"),
    (Action::DismissToasts, HelpSection::Jobs, "Dismiss all active toasts; keep notification history and Jobs evidence"),
    (Action::Notifications, HelpSection::Jobs, "Read noteworthy events and file-operation results; Enter opens linked Jobs evidence"),
    (Action::Help, HelpSection::Application, "Help"),
    (
        Action::Cancel,
        HelpSection::Application,
        "Close overlay, cancel count or clear filter",
    ),
    (
        Action::Quit,
        HelpSection::Application,
        "Quit without saving pane locations",
    ),
    (
        Action::SaveQuit,
        HelpSection::Application,
        "Save pane locations and quit",
    ),
    (
        Action::Diagnostics,
        HelpSection::Application,
        "Toggle read-only diagnostics",
    ),
    (
        Action::DiagnosticsUp,
        HelpSection::Application,
        "Scroll diagnostics up",
    ),
    (
        Action::DiagnosticsDown,
        HelpSection::Application,
        "Scroll diagnostics down",
    ),
}

#[cfg(test)]
mod tests {
    #[test]
    fn each_command_has_one_authoritative_metadata_row() {
        let actions: std::collections::BTreeSet<_> = super::COMMANDS
            .iter()
            .map(|(action, _, _)| action)
            .collect();
        assert_eq!(actions.len(), super::COMMANDS.len());
    }

    #[test]
    fn navigation_starts_with_cursor_movement_in_practical_order() {
        use super::{Action, HelpSection};
        assert_eq!(
            super::metadata(Action::Up),
            (HelpSection::Navigation, "Move cursor up")
        );
        assert_eq!(
            super::metadata(Action::Down),
            (HelpSection::Navigation, "Move cursor down")
        );
    }
}
