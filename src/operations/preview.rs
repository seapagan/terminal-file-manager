use super::{
    Plan,
    safety::{self, Parents, Result},
};
use crate::fs::{Entry, EntryKind};
use std::{collections::HashMap, fs, io, sync::atomic::AtomicBool};

/// A deletion preview is a counted no-follow snapshot, not approval to mutate.
#[derive(Clone, Debug)]
pub struct Preview {
    pub files: u64,
    pub directories: u64,
    pub bytes: u64,
    entries: HashMap<std::path::PathBuf, Entry>,
    parents: Vec<Parents>,
}

impl Preview {
    pub(super) fn approved(&self, path: &std::path::Path) -> Result<Entry> {
        self.entries.get(path).cloned().ok_or_else(|| {
            safety::changed("Unapproved descendant after preview; review a fresh preview")
        })
    }

    pub(super) fn capture(plan: &Plan, cancel: &AtomicBool) -> Result<Self> {
        plan.validate_directories()?;
        let mounts = super::mounts::Mounts::capture(plan.operation)?;
        let mut preview = Self {
            files: 0,
            directories: 0,
            bytes: 0,
            entries: HashMap::new(),
            parents: Vec::new(),
        };
        for source in &plan.operands {
            crate::fs::check_cancelled(cancel)?;
            safety::revalidate(source, true)?;
            preview.parents.push(Parents::capture(&source.path)?);
            preview.scan(source, cancel, 0, &mounts)?;
        }
        for parent in &preview.parents {
            parent.validate()?;
        }
        Ok(preview)
    }

    fn scan(
        &mut self,
        source: &Entry,
        cancel: &AtomicBool,
        depth: usize,
        mounts: &super::mounts::Mounts,
    ) -> Result<()> {
        crate::fs::check_cancelled(cancel)?;
        mounts.reject(source)?;
        if depth > 128 || source.identity.is_none() {
            return Err(io::Error::other("Incomplete recursive preview; deletion disabled").into());
        }
        if source.kind == EntryKind::Directory {
            self.scan_directory(source, cancel, depth, mounts)?;
        } else {
            self.add_file(source)?;
        }
        self.entries.insert(source.path.clone(), source.clone());
        Ok(())
    }

    fn scan_directory(
        &mut self,
        source: &Entry,
        cancel: &AtomicBool,
        depth: usize,
        mounts: &super::mounts::Mounts,
    ) -> Result<()> {
        self.directories = self
            .directories
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Directory count overflow"))?;
        let children: io::Result<Vec<_>> = fs::read_dir(&source.path)?.collect();
        for child in children? {
            self.scan(
                &crate::fs::inspect(&child.path()),
                cancel,
                depth + 1,
                mounts,
            )?;
        }
        Ok(())
    }

    fn add_file(&mut self, source: &Entry) -> Result<()> {
        self.files = self
            .files
            .checked_add(1)
            .ok_or_else(|| io::Error::other("File count overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(source.size.unwrap_or(0))
            .ok_or_else(|| io::Error::other("Size overflow"))?;
        Ok(())
    }

    pub(super) fn validate(&self, plan: &Plan, cancel: &AtomicBool) -> Result<()> {
        for parent in &self.parents {
            parent.validate()?;
        }
        let fresh = Self::capture(plan, cancel)?;
        if self.entries.len() != fresh.entries.len() {
            return Err(safety::changed(
                "Recursive selection changed after preview; review a fresh preview",
            ));
        }
        for (path, before) in &self.entries {
            let Some(after) = fresh.entries.get(path) else {
                return Err(safety::changed("Recursive selection changed"));
            };
            if before.identity != after.identity
                || before.kind != after.kind
                || before.attributes != after.attributes
                || before.link_target != after.link_target
            {
                return Err(safety::changed(
                    "Recursive object changed after preview; fresh review required",
                ));
            }
        }
        Ok(())
    }
}
