//! Virtual navigation rows and path overlays follow their owning pane.
use super::{App, Pane, Request, RequestKind, pane::Visit};
use crate::fs::Identity;
use std::path::{Path, PathBuf};

pub(super) struct ParentRestore {
    visit: Visit,
    waiting: bool,
    installed_revision: Option<u64>,
}

impl App {
    /// Return to the lexical parent, retaining the exited entry's native identity.
    pub fn parent(&mut self) -> Option<Request> {
        let pane = self.active;
        let state = &self.panes[pane];
        let parent = state.path.parent()?.to_path_buf();
        let visit = state.parent_visit(parent.clone());
        // Keep an established context's original identity, including its absence.
        let waiting = state.context == 0 && visit.cursor_identity.is_none();
        let exited = waiting.then(|| state.path.clone());
        let request = self.request_location(pane, parent, RequestKind::ParentListing(exited));
        self.panes[pane].parent_restoring = Some(ParentRestore {
            visit,
            waiting,
            installed_revision: None,
        });
        Some(request)
    }

    pub(crate) fn complete_parent_identity(
        &mut self,
        request: &Request,
        identity: Option<Identity>,
    ) -> bool {
        let Some(state) = self.panes.get_mut(request.pane) else {
            return false;
        };
        if state.generation != request.generation
            || !state
                .parent_restoring
                .as_ref()
                .is_some_and(|restore| restore.waiting)
        {
            return false;
        }
        let mut restore = state.parent_restoring.take().unwrap();
        restore.waiting = false;
        restore.visit.cursor_identity = identity;
        if let Some(revision) = restore.installed_revision {
            if revision != state.selection_revision || !state.restore_parent_visit(&restore.visit) {
                return false;
            }
            state.selection_revision = state.selection_revision.wrapping_add(1);
            self.invalidate_listing_details(request.pane);
            return true;
        }
        state.parent_restoring = Some(restore);
        false
    }

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
    pub(super) fn restore_parent(&mut self) {
        let Some(mut restore) = self.parent_restoring.take() else {
            return;
        };
        if restore.waiting {
            restore.installed_revision = Some(self.selection_revision);
            self.parent_restoring = Some(restore);
        } else {
            self.restore_parent_visit(&restore.visit);
        }
    }

    pub(crate) fn pending_navigation(&self) -> Option<&Path> {
        self.pending.as_deref()
    }

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
