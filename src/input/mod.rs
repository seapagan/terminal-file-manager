//! One action vocabulary for keyboard and mouse dispatch.
mod bindings;
mod click;
mod controller;
mod diagnostics;
mod divider;
mod help;
mod metadata;
mod operation_dialog;
mod operation_exit;
mod operation_jobs;
mod operations;
mod profiles;
mod scroll;
pub(crate) mod view;
pub use bindings::{Bindings, chord};
pub use controller::Controller;

/// Browser and local-operation actions share dispatch; later milestones explain unavailability.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Up,
    Down,
    ExtendUp,
    ExtendDown,
    Home,
    End,
    PageUp,
    PageDown,
    Enter,
    Parent,
    Back,
    Forward,
    Quit,
    SaveQuit,
    Help,
    Diagnostics,
    DiagnosticsUp,
    DiagnosticsDown,
    TogglePanes,
    FocusPane,
    Maximise,
    SplitLeft,
    SplitRight,
    ToggleSidebar,
    Metadata,
    Info,
    ViewOptions,
    ViewProfiles,
    HumanSizes,
    FuzzyTimestamps,
    Permissions,
    OwnerDisplay,
    ColumnSize,
    ColumnModified,
    ColumnAttributes,
    ColumnOwner,
    MetadataPresentation,
    Count,
    Cancel,
    Mark,
    MarkNext,
    SelectAll,
    ClearVisible,
    Filter,
    Hidden,
    Ignore,
    Refresh,
    Sort,
    Reverse,
    Group,
    Viewer,
    Editor,
    Copy,
    Move,
    Mkdir,
    Trash,
    Delete,
    Rename,
    Attributes,
    Jobs,
    CopyQuick,
    MoveQuick,
    Path,
    Zoxide,
    OppositeZoxide,
    ExitDirectory,
}

impl Action {
    pub(super) fn repeatable(self) -> bool {
        matches!(
            self,
            Action::Up
                | Action::Down
                | Action::Home
                | Action::End
                | Action::PageUp
                | Action::PageDown
                | Action::ExtendUp
                | Action::ExtendDown
                | Action::Parent
                | Action::Back
                | Action::Forward
                | Action::SplitLeft
                | Action::SplitRight
                | Action::DiagnosticsUp
                | Action::DiagnosticsDown
        )
    }
}
