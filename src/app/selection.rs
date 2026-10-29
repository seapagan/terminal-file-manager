//! Inclusive visible ranges never take ownership of independent marks.
use super::{Pane, pane::same_object};
use crate::fs::Identity;
use std::{collections::HashSet, path::PathBuf};

pub(super) struct RangeSelection {
    anchor: PathBuf,
    identity: Option<Identity>,
    added: HashSet<PathBuf>,
}

impl Pane {
    pub(super) fn reconcile_range(&mut self) {
        if let Some(range) = &mut self.range {
            range.added.retain(|path| self.marks.contains_key(path));
            if !self.entries.iter().any(|entry| {
                entry.path == range.anchor
                    && entry.error.is_none()
                    && range
                        .identity
                        .as_ref()
                        .zip(entry.identity.as_ref())
                        .is_some_and(|(old, fresh)| {
                            old.object.is_some()
                                && fresh.object.is_some()
                                && same_object(old, fresh)
                        })
            }) {
                self.range = None;
            }
        }
    }

    /// End range ownership without removing any current marks.
    pub fn reset_range(&mut self) {
        self.range = None;
    }

    /// Ordinary movement/click starts a new gesture at this pane's native cursor.
    pub fn anchor_range(&mut self) {
        let identity = self
            .current()
            .filter(|entry| entry.error.is_none())
            .and_then(|entry| entry.identity.clone());
        self.range = self.cursor.clone().map(|anchor| RangeSelection {
            anchor,
            identity,
            added: HashSet::new(),
        });
    }

    /// Extend or shrink the current visible range, retaining pre-existing marks.
    pub fn extend_range(&mut self, delta: isize, height: usize) {
        if self.range.is_none() {
            self.anchor_range();
        }
        self.shift_cursor(delta, height);
        if let Some(path) = self.cursor.clone() {
            self.select_range_to(path, height);
        }
    }

    /// Shift-click uses the same sorted/filtered working set as keyboard ranges.
    pub fn select_range_to(&mut self, path: PathBuf, height: usize) {
        let Some(target) = self
            .visible
            .iter()
            .position(|i| self.entries[*i].path == path)
        else {
            return;
        };
        self.selection_revision = self.selection_revision.wrapping_add(1);
        if self.range.is_none() {
            self.anchor_range();
        }
        let mut range = self.range.take().unwrap_or_else(|| RangeSelection {
            anchor: path.clone(),
            identity: self.entries[self.visible[target]].identity.clone(),
            added: HashSet::new(),
        });
        let anchor = self
            .visible
            .iter()
            .position(|i| self.entries[*i].path == range.anchor);
        if anchor.is_none() {
            range.anchor = path.clone();
            range.identity = self.entries[self.visible[target]].identity.clone();
            range.added.clear();
        }
        let anchor = anchor.unwrap_or(target);
        let indices = &self.visible[anchor.min(target)..=anchor.max(target)];
        let selected: HashSet<_> = indices
            .iter()
            .map(|i| self.entries[*i].path.clone())
            .collect();
        for path in range.added.difference(&selected) {
            self.marks.remove(path);
        }
        range.added.retain(|path| selected.contains(path));
        for i in indices {
            let entry = &self.entries[*i];
            if !self.marks.contains_key(&entry.path)
                && let Some(identity) = &entry.identity
            {
                self.marks.insert(entry.path.clone(), identity.clone());
                range.added.insert(entry.path.clone());
            }
        }
        self.range = Some(range);
        self.cursor = Some(path);
        self.shift_cursor(0, height);
    }
}
