//! Refresh seeds and unchanged-result checks preserve the current working set.
use super::Pane;
use crate::fs::Snapshot;
use std::collections::HashMap;

impl Pane {
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
