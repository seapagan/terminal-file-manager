use super::Action;
use crate::app::App;

pub(super) fn blocked(app: &App, action: Action) -> Option<&'static str> {
    use Action::*;
    let remote = app.panes[app.active].is_remote();
    let opposite = app.panes[1 - app.active].is_remote();
    match action {
        Copy | CopyQuick | Move | MoveQuick if remote || opposite => {
            Some("Transfers involving an SSH pane are unavailable")
        }
        SaveQuit if remote || opposite => Some("An SSH preview cannot be saved as a session"),
        Enter | Parent | Back | Forward | Refresh | FullPath | Metadata | ToggleSidebar | Count
        | Viewer | Editor | PathCopy | Mkdir | CreateFile | Trash | Delete | Rename
        | Attributes | Mark | MarkNext | SelectAll | MarkExtension | MarkFinalExtension
        | ExtendUp | ExtendDown | PatternSelection
            if remote =>
        {
            Some("Only listing controls are available in the read-only SSH pane")
        }
        _ => None,
    }
}
