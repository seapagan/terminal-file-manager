//! Refresh seeds and unchanged-result checks preserve the current working set.
use super::{App, Pane};
use crate::fs::Snapshot;
use std::collections::HashMap;
use std::path::PathBuf;

impl Pane {
    pub(super) fn clear_navigation_filter(&mut self, path: &std::path::Path, keep: bool) {
        if self.path != path && !keep {
            self.filter.clear();
        }
    }

    pub(crate) fn cached_snapshot(&self) -> Snapshot {
        let mut ordered = vec![None; self.entries.len()];
        for entry in &self.entries {
            ordered[self.listing_order[&entry.path]] = Some(entry.clone());
        }
        let entries = ordered.into_iter().flatten().collect();
        Snapshot {
            directory: self.directory.clone().map(Box::new),
            entry_identity: self.entry_identity.clone(),
            path: self.path.clone(),
            entries,
            errors: self.warnings.clone(),
        }
    }
    /// Ignore access-time-only differences when deciding whether to rebuild rows and marks.
    pub(super) fn unchanged_listing(&self, fresh: &Snapshot) -> bool {
        if self.path != fresh.path
            || self.warnings != fresh.errors
            || self.entries.len() != fresh.entries.len()
        {
            return false;
        }
        let directory = self.directory.as_ref().zip(fresh.directory.as_deref());
        if !directory.is_some_and(|(old, new)| old.clone().same_count_inputs(new)) {
            return false;
        }
        let entries: HashMap<_, _> = self
            .entries
            .iter()
            .map(|entry| (&entry.path, entry))
            .collect();
        fresh.entries.iter().enumerate().all(|(index, entry)| {
            self.listing_order.get(&entry.path) == Some(&index)
                && entries
                    .get(&entry.path)
                    .is_some_and(|old| (*old).clone().same_count_inputs(entry))
        })
    }
    pub(super) fn update_access_times(&mut self, fresh: Snapshot) {
        let accessed: HashMap<_, _> = fresh
            .entries
            .into_iter()
            .map(|e| (e.path, e.accessed))
            .collect();
        for entry in &mut self.entries {
            entry.accessed = accessed[&entry.path];
        }
        self.directory = fresh.directory.map(|entry| *entry);
    }
}

impl App {
    pub(super) fn apply_listing(
        &mut self,
        pane: usize,
        attempted: Option<PathBuf>,
        snapshot: Snapshot,
    ) -> bool {
        let state = &mut self.panes[pane];
        let previous = if attempted.is_none() && state.error.is_none() && snapshot.errors.is_empty()
        {
            state.current().cloned()
        } else {
            None
        };
        if attempted.is_none() && state.error.is_none() && state.unchanged_listing(&snapshot) {
            state.update_access_times(snapshot);
            self.reconcile_listing_details(pane, previous);
            return true;
        }
        state.error = snapshot.directory.is_none().then(|| {
            "Directory identity unavailable; reopen the directory before using this context".into()
        });
        if attempted.is_some() || state.context == 0 {
            state.context += 1;
            state.entry_identity = snapshot.entry_identity.clone();
        }
        state.clear_navigation_filter(&snapshot.path, self.keep_filter_on_navigation);
        state.listing_installs += 1;
        state.install(snapshot);
        self.close_stale_path_picker(pane);
        self.reconcile_listing_details(pane, previous);
        true
    }
}
