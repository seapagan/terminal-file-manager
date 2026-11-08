//! Virtual navigation rows and path overlays follow their owning pane.
use super::{App, Pane};
use std::path::PathBuf;

impl App {
    pub(super) fn close_stale_path_picker(&mut self, pane: usize) {
        if self
            .path_picker
            .as_ref()
            .is_some_and(|picker| picker.pane == pane && picker.path != self.panes[pane].path)
        {
            self.path_picker = None;
            self.path_pointer = None;
            if self
                .context_help
                .as_ref()
                .is_some_and(|help| help.topic == crate::input::HelpTopic::FullPath)
            {
                self.context_help = None;
            }
        }
    }
}

impl Pane {
    /// A parent exists according to native lexical path semantics.
    pub fn has_parent(&self) -> bool {
        self.path
            .parent()
            .is_some_and(|p| !p.as_os_str().is_empty())
    }

    /// Parent focus never resolves to a filesystem Entry or operation operand.
    pub fn parent_focused(&self) -> bool {
        self.has_parent() && self.cursor.as_deref() == self.path.parent()
    }

    /// Rendered row count includes the virtual parent, independently of filters.
    pub fn row_count(&self) -> usize {
        self.visible.len() + usize::from(self.has_parent())
    }

    /// Native row target shared by movement and mouse selection.
    pub fn row_path(&self, row: usize) -> Option<PathBuf> {
        if self.has_parent() && row == 0 {
            return self.path.parent().map(PathBuf::from);
        }
        self.visible
            .get(row.checked_sub(usize::from(self.has_parent()))?)
            .map(|i| self.entries[*i].path.clone())
    }

    /// Rendered row index; filesystem cursor_index remains entry-only.
    pub fn row_index(&self) -> Option<usize> {
        if self.parent_focused() {
            Some(0)
        } else {
            self.cursor_index()
                .map(|i| i + usize::from(self.has_parent()))
        }
    }
}
