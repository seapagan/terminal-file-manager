//! SFTP destinations and completion state for the shared pane.
use super::{App, Pane, Request, RequestKind, navigation::record_visit};
use crate::fs::Entry;
use russh_sftp::{client::fs::ReadDir, protocol::SftpString};

pub(super) struct Location {
    path: SftpString,
    pub cursor: Option<SftpString>,
    back: Vec<Visit>,
    forward: Vec<Visit>,
    pending: Option<Navigation>,
}

#[derive(Clone)]
struct Visit {
    path: SftpString,
    cursor: Option<SftpString>,
    scroll: usize,
}

struct Navigation {
    target: SftpString,
    restore: Option<Visit>,
    forward: Option<bool>,
}

impl App {
    /// Install the initial listing in the read-only right pane.
    pub(crate) fn install_sftp(&mut self, path: SftpString, entries: ReadDir) {
        self.install_remote_rows(path, remote_entries(entries));
    }

    fn install_remote_rows(&mut self, path: SftpString, entries: Vec<Entry>) {
        let right = &mut self.panes[1];
        let mut pane = Pane::new(None);
        pane.profile.clone_from(&right.profile);
        pane.profile_modified = right.profile_modified;
        pane.remote = Some(Location {
            path,
            cursor: None,
            back: Vec::new(),
            forward: Vec::new(),
            pending: None,
        });
        pane.listing = false;
        pane.listing_installs = 1;
        pane.entries = entries;
        crate::view::ViewSettings::capture(right).apply(&mut pane);
        *right = pane;
        self.dual = true;
        self.maximized = false;
    }

    pub(crate) fn request_remote_entry(&mut self, pane: usize, path: SftpString) -> Request {
        self.request_remote(pane, path, None, None)
    }

    pub(super) fn remote_parent(&mut self) -> Option<Request> {
        let pane = self.active;
        let state = &self.panes[pane];
        let remote = state.remote.as_ref()?;
        let target = SftpString::from(parent(&remote.path)?);
        let saved = remote
            .back
            .iter()
            .rev()
            .chain(remote.forward.iter().rev())
            .find(|visit| visit.path == target);
        let restore = Visit {
            path: target.clone(),
            cursor: Some(remote.path.clone()),
            scroll: saved.map_or(0, |visit| visit.scroll),
        };
        Some(self.request_remote(pane, target, Some(restore), None))
    }

    pub(super) fn remote_history(&mut self, forward: bool) -> Option<Request> {
        let pane = self.active;
        let remote = self.panes[pane].remote.as_ref()?;
        let visit = if forward {
            remote.forward.last()
        } else {
            remote.back.last()
        }?
        .clone();
        Some(self.request_remote(pane, visit.path.clone(), Some(visit), Some(forward)))
    }

    pub(super) fn remote_refresh(&mut self, pane: usize) -> Option<Request> {
        let state = &mut self.panes[pane];
        let remote = state.remote.as_ref()?;
        if remote.pending.is_some() {
            state.refresh_pending = true;
            return None;
        }
        let path = remote.path.clone();
        Some(self.request_remote_entry(pane, path))
    }

    fn request_remote(
        &mut self,
        pane: usize,
        path: SftpString,
        restore: Option<Visit>,
        forward: Option<bool>,
    ) -> Request {
        let state = &mut self.panes[pane];
        let remote = state.remote.as_mut().expect("remote pane");
        state.generation += 1;
        state.listing = true;
        state.refresh_pending = false;
        remote.pending = Some(Navigation {
            target: path.clone(),
            restore,
            forward,
        });
        Request {
            pane,
            generation: state.generation,
            path: None,
            kind: RequestKind::SftpListing(path),
        }
    }

    /// Install a result only if it still belongs to the pane's current request.
    pub fn complete_sftp(&mut self, request: &Request, result: Result<Vec<Entry>, String>) -> bool {
        let Some(state) = self.panes.get_mut(request.pane) else {
            return false;
        };
        if state.generation != request.generation {
            return false;
        }
        let RequestKind::SftpListing(path) = &request.kind else {
            return false;
        };
        let Some(navigation) = state
            .remote
            .as_mut()
            .and_then(|remote| remote.pending.take_if(|pending| &pending.target == path))
        else {
            return false;
        };
        state.listing = false;
        match result {
            Ok(entries) => {
                state.install_remote(navigation, entries, self.keep_filter_on_navigation)
            }
            Err(error) => self.notify_sftp_failure(request.pane, path, error),
        }
        true
    }

    fn notify_sftp_failure(&mut self, pane: usize, path: &SftpString, error: String) {
        self.panes[pane].refresh_pending = false;
        let previous = self.listing_notices(pane);
        let path = crate::ui::clean(&String::from_utf8_lossy(path.as_bytes()));
        self.panes[pane].error = Some(format!("{path}: {error}"));
        self.retain_listing_notices(pane, &previous);
    }
}

impl Pane {
    /// Whether this pane contains a read-only SFTP listing.
    pub fn is_remote(&self) -> bool {
        self.remote.is_some()
    }

    /// Directory path as sent by SFTP, without native path interpretation.
    pub fn remote_path(&self) -> Option<&[u8]> {
        self.remote
            .as_ref()
            .map(|location| location.path.as_bytes())
    }

    pub(super) fn remote_parent_path(&self) -> Option<&[u8]> {
        parent(&self.remote.as_ref()?.path)
    }

    pub(super) fn remote_row_path(&self, row: usize) -> Option<SftpString> {
        if self.has_parent() && row == 0 {
            return self.remote_parent_path().map(SftpString::from);
        }
        let entry = self
            .visible
            .get(row.checked_sub(usize::from(self.has_parent()))?)?;
        self.entries[*entry]
            .remote
            .as_ref()
            .map(|entry| entry.path.clone())
    }

    fn install_remote(&mut self, navigation: Navigation, entries: Vec<Entry>, keep_filter: bool) {
        let previous = self.cursor_index().unwrap_or(0);
        let remote = self.remote.as_mut().expect("remote pane");
        let navigating = remote.path != navigation.target;
        if navigating {
            let visit = Visit {
                path: remote.path.clone(),
                cursor: remote.cursor.clone(),
                scroll: self.scroll,
            };
            record_visit(
                &mut remote.back,
                &mut remote.forward,
                visit,
                navigation.forward,
            );
            remote.path = navigation.target;
            remote.cursor = parent(&remote.path).map(SftpString::from);
            self.scroll = 0;
            if !keep_filter {
                self.filter.clear();
            }
        }
        if let Some(visit) = navigation.restore {
            remote.cursor = visit.cursor;
            self.scroll = visit.scroll;
        }
        self.entries = entries;
        self.error = None;
        self.listing_installs += 1;
        self.sort.apply(&mut self.entries);
        self.refilter_at(if navigating { 0 } else { previous });
    }
}

// SFTP paths use slash-delimited bytes even when the client is on Windows.
fn parent(path: &SftpString) -> Option<&[u8]> {
    let bytes = path.as_bytes();
    let end = bytes.iter().rposition(|byte| *byte != b'/')? + 1;
    let slash = bytes[..end].iter().rposition(|byte| *byte == b'/')?;
    Some(&bytes[..slash.max(1)])
}

pub(crate) fn remote_entries(entries: ReadDir) -> Vec<Entry> {
    entries
        .enumerate()
        .map(|(order, entry)| Entry::from_sftp(entry, order))
        .collect()
}

#[cfg(test)]
mod tests;
