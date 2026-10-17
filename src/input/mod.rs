//! One action vocabulary for keyboard and mouse dispatch.
mod bindings;
mod controller;
mod help;
pub(crate) mod view;
pub use bindings::{Bindings, chord};
pub use controller::Controller;

/// Only read-only M1 actions execute; later actions explain their unavailability.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Enter,
    Parent,
    Back,
    Forward,
    Quit,
    Help,
    TogglePanes,
    FocusPane,
    ToggleSidebar,
    Metadata,
    Info,
    ViewOptions,
    HumanSizes,
    FuzzyTimestamps,
    Permissions,
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
    Path,
    Zoxide,
    OppositeZoxide,
    ExitDirectory,
}
