//! Formatting belongs to an open metadata view, independently of listing preferences.
use super::{App, Pane};
use crate::{input::Action, owner::OwnerDisplay, permissions::PermissionDisplay};

/// One metadata invocation captures the active pane's formatting defaults.
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

    /// Read a view's formatting, falling back to pane defaults before its first event.
    pub fn metadata_format(&self, floating: bool) -> MetadataFormat {
        self.metadata_formats[usize::from(floating)]
            .filter(|f| f.pane == self.active)
            .unwrap_or_else(|| MetadataFormat::capture(&self.panes[self.active], self.active))
    }
    /// Hidden sidebars close their invocation; a popup has its own independent lifetime.
    pub(crate) fn sync_metadata_views(&mut self, sidebar_visible: bool) {
        for (index, visible) in [sidebar_visible, self.popup].into_iter().enumerate() {
            if !visible {
                self.metadata_formats[index] = None;
            } else if self.metadata_formats[index].is_none_or(|f| f.pane != self.active) {
                self.metadata_formats[index] = Some(MetadataFormat::capture(
                    &self.panes[self.active],
                    self.active,
                ));
            }
        }
    }
}
