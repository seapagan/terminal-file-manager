//! Named view data contains display policy only, never navigation or selection.
use crate::{
    app::{IgnoreMode, Pane},
    listing::{Grouping, Sort, SortPolicy},
    permissions::PermissionDisplay,
};

/// Optional columns still yield to filenames on narrow terminals.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Columns {
    pub size: bool,
    pub modified: bool,
    pub attributes: bool,
    pub owner: bool,
}

impl Default for Columns {
    fn default() -> Self {
        Self {
            size: true,
            modified: true,
            attributes: true,
            owner: true,
        }
    }
}

impl Columns {
    /// One responsive visibility contract for headers and data cells.
    pub fn visible(&self, width: u16) -> Self {
        Self {
            size: self.size && width >= 45,
            modified: self.modified && width >= 70,
            attributes: self.attributes && width >= 90,
            owner: self.owner && width >= 125,
        }
    }
}

/// Serializable view snapshot; applying it preserves native paths and pane identity.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ViewSettings {
    pub human_sizes: bool,
    pub fuzzy_timestamps: bool,
    pub permissions: PermissionDisplay,
    pub columns: Columns,
    pub sort: Sort,
    pub grouping: Grouping,
    pub reverse: bool,
    pub hidden: bool,
    pub ignore: IgnoreMode,
    pub sidebar: Option<bool>,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            human_sizes: true,
            fuzzy_timestamps: false,
            permissions: PermissionDisplay::Symbolic,
            columns: Columns::default(),
            sort: Sort::Natural,
            grouping: Grouping::First,
            reverse: false,
            hidden: false,
            ignore: IgnoreMode::Dim,
            sidebar: None,
        }
    }
}

impl ViewSettings {
    /// Capture policy without locations, cursor, marks, history or filter.
    pub fn capture(pane: &Pane) -> Self {
        Self {
            human_sizes: pane.human_sizes,
            fuzzy_timestamps: pane.fuzzy_timestamps,
            permissions: pane.permissions,
            columns: pane.columns.clone(),
            sort: pane.sort.sort,
            grouping: pane.sort.grouping,
            reverse: pane.sort.reverse,
            hidden: pane.show_hidden,
            ignore: pane.ignore,
            sidebar: None,
        }
    }

    /// Replace only a pane's view policy and recompute its working set/measurements.
    pub fn apply(&self, pane: &mut Pane) {
        pane.human_sizes = self.human_sizes;
        pane.fuzzy_timestamps = self.fuzzy_timestamps;
        pane.permissions = self.permissions;
        pane.columns = self.columns.clone();
        pane.sort = SortPolicy {
            sort: self.sort,
            grouping: self.grouping,
            reverse: self.reverse,
        };
        pane.show_hidden = self.hidden;
        pane.ignore = self.ignore;
        pane.resort();
    }
}
