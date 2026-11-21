//! Native extension classes never use rendered filenames as identity.
use super::Pane;
use crate::fs::{Entry, EntryKind};
use std::path::Path;

fn extension<'a>(entry: &'a Entry, compounds: &'a [String], final_only: bool) -> Option<&'a [u8]> {
    if entry.kind != EntryKind::File || entry.error.is_some() || entry.identity.is_none() {
        return None;
    }
    let name = entry.name.as_encoded_bytes();
    if !final_only
        && let Some(suffix) = compounds
            .iter()
            .filter(|s| name.len() > s.len() && name.ends_with(s.as_bytes()))
            .max_by_key(|s| s.len())
    {
        return Some(suffix.as_bytes());
    }
    Path::new(&entry.name)
        .extension()
        .filter(|e| !e.is_empty())
        .map(|e| e.as_encoded_bytes())
}

impl Pane {
    /// Replace visible marks with the highlighted file's exact or final extension class.
    pub fn mark_extension(&mut self, compounds: &[String], final_only: bool) {
        if self.error.is_some() || self.pending.is_some() {
            return;
        }
        let Some(reference) = self
            .current()
            .and_then(|e| extension(e, compounds, final_only))
        else {
            return;
        };
        let selected: Vec<_> = self
            .visible
            .iter()
            .map(|i| &self.entries[*i])
            .filter(|entry| extension(entry, compounds, final_only) == Some(reference))
            .map(|entry| (entry.path.clone(), entry.identity.clone().unwrap()))
            .collect();
        self.clear_visible();
        self.marks.extend(selected);
    }
}
