use crate::input::Action;

/// Ordered categories in the continuous Help document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpSection {
    Navigation,
    Selection,
    Operations,
    Jobs,
    Panes,
    Views,
    Application,
}

impl HelpSection {
    pub const ALL: [Self; 7] = [
        Self::Navigation,
        Self::Selection,
        Self::Operations,
        Self::Jobs,
        Self::Panes,
        Self::Views,
        Self::Application,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Navigation => "Navigation",
            Self::Selection => "Selection & Marking",
            Self::Operations => "File Operations",
            Self::Jobs => "Jobs & Progress",
            Self::Panes => "Panes & Layout",
            Self::Views => "Views & Metadata",
            Self::Application => "Application & Diagnostics",
        }
    }
}

impl Action {
    /// Exhaustive categorisation keeps new actions attached to the live keymap.
    pub fn help_section(self) -> HelpSection {
        use Action::*;
        match self {
            Up | Down | Home | End | PageUp | PageDown | Enter | Parent | Back | Forward | Path
            | Zoxide | OppositeZoxide | ExitDirectory => HelpSection::Navigation,
            ExtendUp | ExtendDown | Mark | MarkNext | SelectAll | ClearVisible
            | PatternSelection => HelpSection::Selection,
            Copy | Move | CopyQuick | MoveQuick | Mkdir | CreateFile | Trash | Delete | Rename
            | Attributes | Viewer | Editor => HelpSection::Operations,
            Jobs => HelpSection::Jobs,
            TogglePanes | FocusPane | Maximise | SplitLeft | SplitRight => HelpSection::Panes,
            ViewOptions | ViewProfiles | HumanSizes | FuzzyTimestamps | Permissions
            | OwnerDisplay | ColumnSize | ColumnModified | ColumnAttributes | ColumnOwner
            | MetadataPresentation | ToggleSidebar | Metadata | Info | Count | Filter | Hidden
            | Ignore | Sort | Reverse | Group => HelpSection::Views,
            Quit | SaveQuit | Help | Diagnostics | DiagnosticsUp | DiagnosticsDown | Cancel
            | Refresh => HelpSection::Application,
        }
    }
}
