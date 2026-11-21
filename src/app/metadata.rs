//! Metadata surfaces share session formatting per pane, independently of listing preferences.
use super::{App, Pane};
use crate::{input::Action, owner::OwnerDisplay, permissions::PermissionDisplay};

/// Both metadata surfaces use this pane's session formatting choices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataFormat {
    pane: usize,
    pub permissions: PermissionDisplay,
    pub fuzzy_timestamps: bool,
    pub owner_display: OwnerDisplay,
}

impl MetadataFormat {
    fn capture(pane: &Pane, id: usize) -> Self {
        Self {
            pane: id,
            permissions: pane.permissions,
            fuzzy_timestamps: pane.fuzzy_timestamps,
            owner_display: pane.owner_display,
        }
    }
    pub(crate) fn toggle(&mut self, action: Action) {
        match action {
            Action::Permissions => {
                self.permissions = match self.permissions {
                    PermissionDisplay::Symbolic => PermissionDisplay::Octal,
                    PermissionDisplay::Octal => PermissionDisplay::Symbolic,
                }
            }
            Action::FuzzyTimestamps => self.fuzzy_timestamps = !self.fuzzy_timestamps,
            Action::OwnerDisplay => {
                self.owner_display = match self.owner_display {
                    OwnerDisplay::Names => OwnerDisplay::Numeric,
                    OwnerDisplay::Numeric => OwnerDisplay::Names,
                }
            }
            _ => {}
        }
    }
}

impl App {
    /// Effective sidebar availability preserves the requested pane preference.
    pub fn sidebar_visible(&self, width: u16) -> bool {
        self.sidebar && !self.dual && width >= 100
    }

    /// Read shared pane formatting, falling back to listing defaults before first use.
    pub fn metadata_format(&self, _floating: bool) -> MetadataFormat {
        self.metadata_formats[self.active]
            .filter(|f| f.pane == self.active)
            .unwrap_or_else(|| MetadataFormat::capture(&self.panes[self.active], self.active))
    }
    /// Either visible surface initializes the same pane-local formatting state.
    pub(crate) fn sync_metadata_views(&mut self, sidebar_visible: bool) {
        if (sidebar_visible || self.popup) && self.metadata_formats[self.active].is_none() {
            self.metadata_formats[self.active] = Some(MetadataFormat::capture(
                &self.panes[self.active],
                self.active,
            ));
        }
    }
}
