//! Authoritative pane state and pure transitions. No filesystem calls.
mod metadata;
mod pane;
pub use metadata::MetadataFormat;
mod selection;
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
    pub totals_generation: u64,
    pub counting: bool,
}

/// Both panes remain alive when hidden or temporarily collapsed by width.
pub struct App {
    pub panes: [Pane; 2],
    pub active: usize,
    pub dual: bool,
    pub maximized: bool,
    pub split_percent: u16,
    pub sidebar: bool,
    pub sidebar_left: bool,
    pub popup: bool,
    pub help: bool,
    pub diagnostics: bool,
    pub diagnostics_scroll: u16,
    pub view_menu: Option<usize>,
    pub profile_menu: Option<usize>,
    pub profile_choice: usize,
    pub profile_name: Option<String>,
    pub profile_names: Vec<String>,
    pub metadata_presentation: crate::settings::MetadataPresentation,
    pub help_rows: Vec<(String, String)>,
    pub overlay_scroll: u16,
    pub metadata_scroll: u16,
    pub metadata_formats: [Option<MetadataFormat>; 2],
    pub status: String,
    pub now: std::time::SystemTime,
    pub details: [Detail; 2],
}

impl App {
    /// Every invocation starts single-pane, with two independent original locations.
    pub fn new(path: PathBuf) -> Self {
        Self {
            panes: [Pane::new(path.clone()), Pane::new(path)],
            active: 0,
            dual: false,
            maximized: false,
            split_percent: 50,
            sidebar: false,
            sidebar_left: false,
            popup: false,
            help: false,
            diagnostics: false,
            diagnostics_scroll: 0,
            view_menu: None,
            profile_menu: None,
            profile_choice: 0,
            profile_name: None,
            profile_names: Vec::new(),
            metadata_presentation: crate::settings::MetadataPresentation::default(),
            help_rows: crate::input::Bindings::new(&Default::default())
                .expect("default bindings")
                .help_rows(),
            overlay_scroll: 0,
            metadata_scroll: 0,
            metadata_formats: [None; 2],
            status: "Read-only — ? for help".into(),
            now: std::time::SystemTime::now(),
            details: std::array::from_fn(|_| Detail::default()),
        }
    }

    /// Effective width never changes the user's requested layout.
    pub fn visible_panes(&self, width: u16) -> Vec<usize> {
        if self.dual && !self.maximized && width >= 80 {
            vec![0, 1]
        } else {
            vec![self.active]
        }
    }

    /// Request navigation; commit the location/history only on successful completion.
    pub fn request_listing(&mut self, pane: usize, path: PathBuf) -> Request {
        let state = &mut self.panes[pane];
        state.reset_range();
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
                self.details[pane].totals_generation += 1;
                self.details[pane].counting = false;
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
        if pane == self.active {
            self.metadata_scroll = 0;
        }
        detail.generation += 1;
        detail.path = path.clone();
        detail.entry = None;
        detail.canonical = None;
        detail.totals = None;
        detail.totals_generation += 1;
        detail.counting = false;
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

    /// Start an explicit count using a generation independent of metadata loading.
    pub fn request_totals(&mut self) -> Option<Request> {
        let pane = self.active;
        let entry = self.panes[pane].current()?;
        if !entry.may_recurse() {
            return None;
        }
        let detail = &mut self.details[pane];
        detail.totals_generation += 1;
        detail.counting = true;
        detail.totals = None;
        Some(Request {
            pane,
            generation: detail.totals_generation,
            path: entry.path.clone(),
            kind: RequestKind::Totals,
        })
    }

    /// Ignore old/cancelled count results even if the cursor returns to the same path.
    pub fn complete_totals(&mut self, request: &Request, result: Result<Totals, String>) -> bool {
        if !self.panes.get(request.pane).is_some_and(|pane| {
            pane.current()
                .is_some_and(|entry| entry.path == request.path)
        }) {
            return false;
        }
        let Some(detail) = self.details.get_mut(request.pane) else {
            return false;
        };
        if detail.totals_generation != request.generation
            || detail.path.as_ref() != Some(&request.path)
        {
            return false;
        }
        detail.counting = false;
        detail.totals = Some(result);
        true
    }

    /// Report worker failures through the same generation checks as successful work.
    pub fn complete_failure(&mut self, request: &Request, error: String) -> bool {
        match request.kind {
            RequestKind::Listing => {
                self.complete_listing(request.pane, request.generation, Err(error))
            }
            RequestKind::Totals => self.complete_totals(request, Err(error)),
            RequestKind::Metadata => {
                if !self.detail_matches(request) {
                    return false;
                }
                let mut entry = self.panes[request.pane].current().unwrap().clone();
                entry.error = Some(error);
                self.complete_metadata(request, entry, None)
            }
        }
    }

    /// Cancel counts in state immediately; the worker cooperatively stops traversal.
    pub fn cancel_totals(&mut self) {
        let detail = &mut self.details[self.active];
        detail.totals_generation += 1;
        detail.counting = false;
        detail.totals = None;
    }
}
