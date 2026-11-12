//! Cached lexical and physical directory evidence; native checks run only in workers.
use crate::{
    app::Pane,
    fs::{Entry, Identity},
};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) struct Directory {
    path: PathBuf,
    lexical: Identity,
    physical: Entry,
}

impl Directory {
    pub(super) fn capture(pane: &Pane) -> Result<Self, String> {
        let physical = pane
            .directory
            .as_ref()
            .filter(|entry| entry.enterable && entry.error.is_none() && entry.identity.is_some())
            .ok_or("Wait for a valid local directory listing before copying paths")?;
        let lexical = pane
            .entry_identity
            .clone()
            .ok_or("Current directory identity is unavailable; refresh first")?;
        Ok(Self {
            path: pane.path.clone(),
            lexical,
            physical: physical.clone(),
        })
    }

    pub(super) fn entry(&self) -> Entry {
        let mut entry = self.physical.clone();
        entry.path = self.path.clone();
        entry.identity = Some(self.lexical.clone());
        entry
    }

    pub(super) fn validate(&self, inspect: &mut impl FnMut(&Path) -> Entry) -> Result<(), String> {
        let lexical = inspect(&self.path);
        let physical = inspect(&self.physical.path);
        if lexical.error.is_some()
            || physical.error.is_some()
            || !lexical
                .identity
                .as_ref()
                .is_some_and(|fresh| super::same_identity(&self.lexical, fresh))
            || std::fs::canonicalize(&self.path).ok().as_ref() != Some(&self.physical.path)
            || !super::same_entry(&self.physical, &physical)
        {
            return Err(
                "Current directory was removed or replaced; refresh and start again".into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
