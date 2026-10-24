//! Listing context composes the shared palette before cursor decoration.
use super::{Role, Theme};
use crate::fs::Entry;
use ratatui::style::Style;

impl Theme {
    /// Keep faint effective on VTE, which ignores it on direct RGB foregrounds.
    /// Cursor styling is applied afterwards and retains precedence.
    pub fn ignored_name_style(&self, entry: &Entry, active: bool) -> Style {
        let style = self.entry_style(entry).patch(self.style(Role::Ignored));
        let inherited = active
            .then(|| self.style(Role::ActivePane))
            .and_then(|s| s.fg);
        self.faint_style(style, inherited)
    }

    /// VTE needs indexed cyan for faint hidden markers; blank cells keep their style.
    pub(crate) fn hidden_indicator_style(&self, hidden: bool) -> Style {
        let style = self.style(Role::Hidden);
        if hidden {
            self.faint_style(style, None)
        } else {
            style
        }
    }
}
