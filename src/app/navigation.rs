//! Virtual navigation rows and path overlays follow their owning pane.
use super::{App, Pane, Request, RequestKind, pane::Visit};
use crate::fs::{Entry, Identity};
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
        if state.is_remote() {
            return self.remote_parent();
        }
        let parent = state.local_path().parent()?.to_path_buf();
        let visit = state.parent_visit(parent.clone());
        // Keep an established context's original identity, including its absence.
        let waiting = state.context == 0 && visit.cursor_identity.is_none();
        let exited = waiting.then(|| state.local_path().clone());
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
        if self.path_picker.as_ref().is_some_and(|picker| {
            picker.pane == pane && Some(&picker.path) != self.panes[pane].native_path()
        }) {
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
        if self.is_remote() {
            return self.remote_parent_path().is_some();
        }
        self.native_path()
            .and_then(|path| path.parent())
            .is_some_and(|p| !p.as_os_str().is_empty())
    }

    /// Parent focus never resolves to a filesystem Entry or operation operand.
    pub fn parent_focused(&self) -> bool {
        if let Some(remote) = &self.remote {
            return self.remote_parent_path().is_some_and(|parent| {
                remote
                    .cursor
                    .as_ref()
                    .map(russh_sftp::protocol::SftpString::as_bytes)
                    == Some(parent)
            });
        }
        self.has_parent() && self.cursor.as_deref() == self.local_path().parent()
    }

    /// Rendered row count includes the virtual parent, independently of filters.
    pub fn row_count(&self) -> usize {
        self.visible.len() + usize::from(self.has_parent())
    }

    /// Native row target shared by movement and mouse selection.
    pub fn row_path(&self, row: usize) -> Option<PathBuf> {
        if self.is_remote() {
            return None;
        }
        if self.has_parent() && row == 0 {
            return self.local_path().parent().map(PathBuf::from);
        }
        self.visible
            .get(row.checked_sub(usize::from(self.has_parent()))?)
            .and_then(|i| self.entries[*i].path.clone())
    }

    pub(crate) fn entry_focused(&self, entry: &Entry) -> bool {
        if let Some(remote) = &self.remote {
            remote
                .cursor
                .as_ref()
                .is_some_and(|cursor| Some(cursor.as_bytes()) == entry.remote_path())
        } else {
            self.cursor.as_ref() == entry.native_path()
        }
    }

    pub(super) fn set_entry_focus(&mut self, index: Option<usize>) {
        let entry = index.and_then(|i| self.entries.get(i));
        if let Some(remote) = &mut self.remote {
            remote.cursor = entry
                .and_then(|e| e.remote.as_ref())
                .map(|r| r.path.clone());
        } else {
            self.cursor = entry.map(|e| e.local_path().clone());
        }
    }

    pub(crate) fn navigation_target(&self, row: usize) -> Option<super::NavigationTarget> {
        if self.is_remote() {
            self.remote_row_path(row).map(super::NavigationTarget::Sftp)
        } else {
            self.row_path(row).map(super::NavigationTarget::Native)
        }
    }

    pub(crate) fn focus_row(&mut self, row: usize) {
        if self.is_remote() {
            let path = self.remote_row_path(row);
            self.remote.as_mut().expect("remote pane").cursor = path;
        } else {
            self.cursor = self.row_path(row);
        }
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

pub(super) fn record_visit<T>(
    back: &mut Vec<T>,
    forward: &mut Vec<T>,
    visit: T,
    direction: Option<bool>,
) {
    match direction {
        Some(true) => {
            forward.pop();
            back.push(visit);
        }
        Some(false) => {
            back.pop();
            forward.push(visit);
        }
        None => {
            back.push(visit);
            forward.clear();
        }
    }
}
