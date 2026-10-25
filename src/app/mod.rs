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
    pub stale: Option<String>,
    pub generation: u64,
    pub path: Option<PathBuf>,
    pub entry: Option<Entry>,
    pub canonical: Option<PathBuf>,
    pub canonical_pending: bool,
    pub totals: Option<Result<Totals, String>>,
    pub totals_generation: u64,
    pub counting: bool,
}

/// Both panes remain alive when hidden or temporarily collapsed by width.
pub struct App {
    pub operations: crate::operations::State,
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
            operations: Default::default(),
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
            status: "Local files — ? for help, F9 / Alt+J for jobs".into(),
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
        if state.pending.is_some() && state.refresh_pending {
            self.status = "Deferred refresh cancelled by newer navigation".into();
        }
        state.reset_range();
        state.generation += 1;
        state.pending = Some(path.clone());
        state.error = None;
        state.restoring = None;
        state.parent_restoring = None;
        state.listing = true;
        state.refresh_pending = false;
        Request {
            pane,
            generation: state.generation,
            path,
            kind: RequestKind::Listing,
        }
    }

    /// Return to the lexical parent, retaining the exited entry's native identity.
    pub fn parent(&mut self) -> Option<Request> {
        let pane = &self.panes[self.active];
        let parent = pane.path.parent()?.to_path_buf();
        let visit = pane.parent_visit(parent.clone());
        let request = self.request_listing(self.active, parent);
        self.panes[self.active].parent_restoring = Some(visit);
        Some(request)
    }

    /// Refresh explicitly, or defer once to the pending navigation's successful target.
    /// Failed or superseding navigation cancels that deferred intent.
    pub fn request_refresh(&mut self, pane: usize) -> Option<Request> {
        let state = self.panes.get_mut(pane)?;
        if let Some(path) = &state.pending {
            state.refresh_pending = true;
            self.status = format!("Refresh queued for {} after navigation", path.display());
            return None;
        }
        self.invalidate_listing_details(pane);
        self.request_reconciliation(pane)
    }

    /// Reconcile without stealing navigation or cancelling a scan or unchanged count.
    pub(crate) fn request_reconciliation(&mut self, pane: usize) -> Option<Request> {
        let state = self.panes.get_mut(pane)?;
        if state.pending.is_some() {
            return None;
        }
        state.refresh_pending = true;
        self.take_refresh(pane)
    }

    /// Start one deferred ordinary refresh after the current listing has completed.
    pub fn take_refresh(&mut self, pane: usize) -> Option<Request> {
        let state = self.panes.get_mut(pane)?;
        if !state.refresh_pending || state.listing || state.pending.is_some() {
            return None;
        }
        state.refresh_pending = false;
        state.listing = true;
        state.generation += 1;
        Some(Request {
            pane,
            generation: state.generation,
            path: state.path.clone(),
            kind: RequestKind::Listing,
        })
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
        state.listing = false;
        match result {
            Ok(snapshot) => self.install_listing(pane, attempted, snapshot),
            Err(error) => {
                if attempted.is_some() && state.refresh_pending {
                    state.refresh_pending = false;
                    self.status = "Deferred refresh cancelled because navigation failed".into();
                }
                state.restoring = None;
                state.parent_restoring = None;
                state.error = Some(format!(
                    "{}: {error}",
                    attempted.as_deref().unwrap_or(&state.path).display()
                ));
                self.invalidate_listing_details(pane);
                true
            }
        }
    }

    fn install_listing(
        &mut self,
        pane: usize,
        attempted: Option<PathBuf>,
        snapshot: Snapshot,
    ) -> bool {
        let state = &mut self.panes[pane];
        if attempted
            .as_ref()
            .is_some_and(|path| path != &snapshot.path)
        {
            state.refresh_pending = false;
            self.status = "Navigation result rejected; deferred refresh cancelled".into();
            state.restoring = None;
            state.parent_restoring = None;
            return false;
        }
        if attempted.is_none() && !state.accepts_refresh(&snapshot) {
            state.error = Some("Directory identity changed or unavailable; reopen the directory to select a new context".into());
            self.invalidate_listing_details(pane);
            return true;
        }
        let previous = if attempted.is_none() && state.error.is_none() && snapshot.errors.is_empty()
        {
            state.current().cloned()
        } else {
            None
        };
        state.error = snapshot.directory.is_none().then(|| {
            "Directory identity unavailable; reopen the directory before using this context".into()
        });
        if attempted.is_some() || state.context == 0 {
            state.context += 1;
        }
        state.install(snapshot);
        self.reconcile_listing_details(pane, previous);
        true
    }

    fn reconcile_listing_details(&mut self, pane: usize, previous: Option<Entry>) {
        let state = &self.panes[pane];
        let unchanged = previous
            .zip(state.current())
            .is_some_and(|(mut old, fresh)| {
                // Enumeration/counting can change access time without changing count inputs.
                old.accessed = fresh.accessed;
                old.identity.is_some() && old.error.is_none() && old == *fresh
            });
        if !unchanged {
            self.invalidate_listing_details(pane);
        } else if let Some(entry) = self.details[pane].entry.as_mut() {
            entry.accessed = state.current().unwrap().accessed;
        }
    }

    fn invalidate_listing_details(&mut self, pane: usize) {
        if pane == self.active {
            self.metadata_scroll = 0;
            if self.popup {
                self.overlay_scroll = 0;
            }
        }
        let detail = &mut self.details[pane];
        detail.stale = self.panes[pane].error.clone();
        detail.generation += 1;
        detail.entry = None;
        detail.canonical = None;
        detail.canonical_pending = false;
        detail.totals = None;
        detail.totals_generation += 1;
        detail.counting = false;
    }

    /// Coalesce metadata work by current native path and selection generation.
    pub fn request_metadata(&mut self, pane: usize) -> Option<Request> {
        if self.panes[pane].error.is_some() {
            return None;
        }
        let entry = self.panes[pane].current();
        let path = entry.map(|entry| entry.path.clone());
        let detail = &mut self.details[pane];
        if path == detail.path && (detail.entry.is_some() || path.is_none()) {
            return None;
        }
        if pane == self.active && path != detail.path {
            self.metadata_scroll = 0;
        }
        detail.generation += 1;
        detail.path = path.clone();
        detail.entry = entry.cloned();
        detail.canonical = None;
        detail.canonical_pending = path.is_some();
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
        detail.canonical_pending = false;
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
        if self.panes[pane].error.is_some() {
            return None;
        }
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
        if !detail.counting {
            return;
        }
        detail.totals_generation += 1;
        detail.counting = false;
        detail.totals = None;
    }
}
