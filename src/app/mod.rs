//! Authoritative pane state and pure transitions. No filesystem calls.
mod pane;
use crate::fs::{Entry, Snapshot, Totals};
pub use pane::{IgnoreMode, Pane, SelectionCounts};
use std::path::PathBuf;

/// A bounded worker request carries identity independently of display text.
#[derive(Clone, Debug)]
pub struct Request {
    pub pane: usize,
    pub generation: u64,
    pub path: PathBuf,
    pub kind: RequestKind,
}

/// Independent request lanes keep slow counts from starving listings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestKind {
    Listing,
    Metadata,
    Totals,
}

/// Metadata cache shared by sidebar and popup for each pane.
#[derive(Clone, Debug, Default)]
pub struct Detail {
    pub generation: u64,
    pub path: Option<PathBuf>,
    pub entry: Option<Entry>,
    pub canonical: Option<PathBuf>,
    pub totals: Option<Result<Totals, String>>,
}

/// Both panes remain alive when hidden or temporarily collapsed by width.
pub struct App {
    pub panes: [Pane; 2],
    pub active: usize,
    pub dual: bool,
    pub sidebar: bool,
    pub sidebar_left: bool,
    pub popup: bool,
    pub help: bool,
    pub status: String,
    pub details: [Detail; 2],
}

impl App {
    /// Every invocation starts single-pane, with two independent original locations.
    pub fn new(path: PathBuf) -> Self {
        Self {
            panes: [Pane::new(path.clone()), Pane::new(path)],
            active: 0,
            dual: false,
            sidebar: false,
            sidebar_left: false,
            popup: false,
            help: false,
            status: "Read-only — ? for help".into(),
            details: std::array::from_fn(|_| Detail::default()),
        }
    }

    /// Effective width never changes the user's requested layout.
    pub fn visible_panes(&self, width: u16) -> Vec<usize> {
        if self.dual && width >= 80 {
            vec![0, 1]
        } else {
            vec![self.active]
        }
    }

    /// Request navigation; commit the location/history only on successful completion.
    pub fn request_listing(&mut self, pane: usize, path: PathBuf) -> Request {
        let state = &mut self.panes[pane];
        state.generation += 1;
        state.pending = Some(path.clone());
        state.error = None;
        state.restoring = None;
        Request {
            pane,
            generation: state.generation,
            path,
            kind: RequestKind::Listing,
        }
    }

    /// Reject stale or invalid pane results, retaining the last good listing on error.
    pub fn complete_listing(
        &mut self,
        pane: usize,
        generation: u64,
        result: Result<Snapshot, String>,
    ) -> bool {
        let Some(state) = self.panes.get_mut(pane) else {
            return false;
        };
        if generation != state.generation {
            return false;
        }
        let attempted = state.pending.take();
        match result {
            Ok(snapshot) => {
                if attempted
                    .as_ref()
                    .is_some_and(|path| path != &snapshot.path)
                {
                    return false;
                }
                state.install(snapshot);
                self.details[pane].generation += 1;
                self.details[pane].path = None;
                self.details[pane].entry = None;
                self.details[pane].totals = None;
            }
            Err(error) => {
                state.restoring = None;
                state.error = Some(format!(
                    "{}: {error}",
                    attempted.as_deref().unwrap_or(&state.path).display()
                ));
            }
        }
        true
    }

    /// Coalesce metadata work by current native path and selection generation.
    pub fn request_metadata(&mut self, pane: usize) -> Option<Request> {
        let path = self.panes[pane].current().map(|e| e.path.clone());
        let detail = &mut self.details[pane];
        if path == detail.path {
            return None;
        }
        detail.generation += 1;
        detail.path = path.clone();
        detail.entry = None;
        detail.canonical = None;
        detail.totals = None;
        path.map(|path| Request {
            pane,
            generation: detail.generation,
            path,
            kind: RequestKind::Metadata,
        })
    }

    /// A late selection result cannot overwrite another entry's metadata.
    pub fn complete_metadata(
        &mut self,
        request: &Request,
        entry: Entry,
        canonical: Option<PathBuf>,
    ) -> bool {
        if !self.detail_matches(request) {
            return false;
        }
        let detail = &mut self.details[request.pane];
        detail.entry = Some(entry);
        detail.canonical = canonical;
        true
    }

    pub(crate) fn detail_matches(&self, request: &Request) -> bool {
        self.details.get(request.pane).is_some_and(|detail| {
            detail.generation == request.generation
                && detail.path.as_ref() == Some(&request.path)
                && self.panes[request.pane]
                    .current()
                    .is_some_and(|entry| entry.path == request.path)
        })
    }

    /// Navigate session history without consuming an entry until the read succeeds.
    pub fn history(&mut self, forward: bool) -> Option<Request> {
        let pane = &self.panes[self.active];
        let visit = if forward {
            pane.forward.last()
        } else {
            pane.back.last()
        }?
        .clone();
        let request = self.request_listing(self.active, visit.path.clone());
        self.panes[self.active].restoring = Some(visit);
        self.panes[self.active].history_forward = forward;
        Some(request)
    }
}
