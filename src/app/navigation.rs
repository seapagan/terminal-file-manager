//! Virtual navigation rows are separate from the filesystem working set.
use super::Pane;
use std::path::PathBuf;

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
