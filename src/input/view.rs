use super::{Action, Controller};
use crate::{
    app::App, permissions::PermissionDisplay, settings::MetadataPresentation, ui::Geometry,
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

/// Menu rows dispatch the same actions as direct shortcuts.
pub(crate) const OPTIONS: &[(&str, Action)] = &[
    ("Human sizes", Action::HumanSizes),
    ("Fuzzy timestamps", Action::FuzzyTimestamps),
    ("Permissions", Action::Permissions),
    ("Hidden files", Action::Hidden),
    ("Git-ignore", Action::Ignore),
    ("Directory grouping", Action::Group),
    ("Sort", Action::Sort),
    ("Reverse sort", Action::Reverse),
    ("Size column", Action::ColumnSize),
    ("Modified column", Action::ColumnModified),
    ("Attributes column", Action::ColumnAttributes),
    ("Owner column", Action::ColumnOwner),
    ("Metadata sidebar", Action::ToggleSidebar),
    ("Preferred metadata", Action::MetadataPresentation),
    ("Owner/group display", Action::OwnerDisplay),
];

pub(super) fn key(controller: &mut Controller, key: KeyEvent, geometry: &Geometry) {
    let index = controller.app.view_menu.as_mut().expect("view menu");
    match key.code {
        KeyCode::Up => *index = index.saturating_sub(1),
        KeyCode::Down => *index = (*index + 1).min(OPTIONS.len() - 1),
        KeyCode::Enter | KeyCode::Char(' ') => {
            let action = OPTIONS[*index].1;
            controller.dispatch(action, geometry);
        }
        KeyCode::Esc => controller.app.view_menu = None,
        _ => {}
    }
}

pub(super) fn mouse(controller: &mut Controller, event: MouseEvent, geometry: &Geometry) {
    let Some(area) = geometry.menu else {
        return;
    };
    let index = controller.app.view_menu.as_mut().expect("view menu");
    match event.kind {
        MouseEventKind::ScrollUp => *index = index.saturating_sub(1),
        MouseEventKind::ScrollDown => *index = (*index + 1).min(OPTIONS.len() - 1),
        MouseEventKind::Down(MouseButton::Left)
            if area.contains((event.column, event.row).into()) =>
        {
            if event.row <= area.y || event.row >= area.bottom().saturating_sub(1) {
                return;
            }
            let row = usize::from(event.row - area.y - 1);
            let start = index.saturating_sub(usize::from(area.height.saturating_sub(3)));
            *index = (start + row).min(OPTIONS.len() - 1);
            let action = OPTIONS[*index].1;
            controller.dispatch(action, geometry);
        }
        _ => {}
    }
}

pub(super) fn formatting(controller: &mut Controller, action: Action) {
    let pane = &mut controller.app.panes[controller.app.active];
    match action {
        Action::HumanSizes => pane.human_sizes = !pane.human_sizes,
        Action::FuzzyTimestamps => pane.fuzzy_timestamps = !pane.fuzzy_timestamps,
        Action::Permissions => {
            pane.permissions = match pane.permissions {
                PermissionDisplay::Symbolic => PermissionDisplay::Octal,
                PermissionDisplay::Octal => PermissionDisplay::Symbolic,
            }
        }
        Action::OwnerDisplay => {
            pane.owner_display = match pane.owner_display {
                crate::owner::OwnerDisplay::Names => crate::owner::OwnerDisplay::Numeric,
                crate::owner::OwnerDisplay::Numeric => crate::owner::OwnerDisplay::Names,
            };
        }
        Action::ColumnSize => pane.columns.size = !pane.columns.size,
        Action::ColumnModified => pane.columns.modified = !pane.columns.modified,
        Action::ColumnAttributes => pane.columns.attributes = !pane.columns.attributes,
        Action::ColumnOwner => pane.columns.owner = !pane.columns.owner,
        Action::MetadataPresentation => {
            controller.preferences.metadata_presentation =
                match controller.preferences.metadata_presentation {
                    MetadataPresentation::Sidebar => MetadataPresentation::Popup,
                    MetadataPresentation::Popup => MetadataPresentation::Sidebar,
                }
        }
        _ => {}
    }
    if action != Action::MetadataPresentation {
        pane.profile_modified = pane.profile.is_some();
    }
    pane.refilter();
    controller.app.metadata_presentation = controller.preferences.metadata_presentation;
    controller.preferences.capture(&controller.app, action);
    controller.preferences_changed = true;
}

pub(crate) fn rows(app: &App, preferred: MetadataPresentation) -> Vec<(String, String)> {
    let pane = &app.panes[app.active];
    let check = |value| if value { "[x]".into() } else { "[ ]".into() };
    let values = vec![
        check(pane.human_sizes),
        check(pane.fuzzy_timestamps),
        format!("{:?}", pane.permissions),
        check(pane.show_hidden),
        format!("{:?}", pane.ignore),
        format!("{:?}", pane.sort.grouping),
        format!("{:?}", pane.sort.sort),
        check(pane.sort.reverse),
        check(pane.columns.size),
        check(pane.columns.modified),
        check(pane.columns.attributes),
        check(pane.columns.owner),
        check(app.sidebar),
        format!("{preferred:?}"),
        format!("{:?}", pane.owner_display),
    ];
    OPTIONS
        .iter()
        .zip(values)
        .map(|((label, _), value)| ((*label).into(), value))
        .collect()
}
