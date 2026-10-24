use super::{
    Plan,
    safety::{self, Parents, Result},
};
use crate::fs::{Entry, EntryKind};
use std::{
    collections::{HashMap, HashSet},
    fs, io,
    sync::atomic::AtomicBool,
};

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
    pub(super) fn claim_objects(&self, root: &Entry) -> io::Result<HashSet<(u64, u64)>> {
        safety::revalidate(root, false).map_err(|error| io::Error::other(error.detail))?;
        if !self
            .entries
            .get(&root.path)
            .is_some_and(|entry| same_entry(entry, root))
        {
            return Err(io::Error::other(
                "Claim differs from immutable approved root",
            ));
        }
        self.entries
            .values()
            .filter(|entry| entry.path.starts_with(&root.path))
            .map(|entry| {
                entry
                    .identity
                    .as_ref()
                    .and_then(|id| id.object)
                    .ok_or_else(|| io::Error::other("Approved native alias identity unavailable"))
            })
            .collect()
    }

    pub(super) fn inventory(&self, root: &std::path::Path) -> (u64, u64) {
        if self
            .entries
            .get(root)
            .is_some_and(|entry| !entry.may_recurse())
        {
            return (1, 0);
        }
        self.entries
            .values()
            .filter(|entry| entry.path.starts_with(root))
            .fold((0, 0), |(files, directories), entry| {
                if entry.kind == EntryKind::Directory {
                    (files, directories + 1)
                } else {
                    (files + 1, directories)
                }
            })
    }

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
            if !same_entry(before, after) {
                return Err(safety::changed(
                    "Recursive object changed after preview; fresh review required",
                ));
            }
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub(super) fn validate_root(&self, root: &Entry, cancel: &AtomicBool) -> Result<()> {
        for parent in &self.parents {
            parent.validate()?;
        }
        let mounts = super::mounts::Mounts::capture(super::Operation::Trash)?;
        let count = self.validate_entry(&crate::fs::inspect(&root.path), cancel, &mounts, 0)?;
        let expected = if root.may_recurse() {
            self.entries
                .keys()
                .filter(|path| path.starts_with(&root.path))
                .count()
        } else {
            1
        };
        if count != expected {
            return Err(safety::changed(
                "Recursive scope membership changed after preview",
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn validate_entry(
        &self,
        fresh: &Entry,
        cancel: &AtomicBool,
        mounts: &super::mounts::Mounts,
        depth: usize,
    ) -> Result<usize> {
        crate::fs::check_cancelled(cancel)?;
        mounts.reject(fresh)?;
        let before = self
            .entries
            .get(&fresh.path)
            .ok_or_else(|| safety::changed("Unapproved descendant after preview"))?;
        if depth > 128 || !same_entry(before, fresh) {
            return Err(safety::changed(
                "Recursive object changed after preview; fresh review required",
            ));
        }
        let mut count = 1;
        if fresh.may_recurse() {
            count += self.validate_children(fresh, cancel, mounts, depth)?;
            safety::revalidate(fresh, true)?;
        }
        Ok(count)
    }

    #[cfg(target_os = "linux")]
    fn validate_children(
        &self,
        root: &Entry,
        cancel: &AtomicBool,
        mounts: &super::mounts::Mounts,
        depth: usize,
    ) -> Result<usize> {
        let mut count = 0;
        for child in fs::read_dir(&root.path)? {
            count += self.validate_entry(
                &crate::fs::inspect(&child?.path()),
                cancel,
                mounts,
                depth + 1,
            )?;
        }
        Ok(count)
    }
}

fn same_entry(before: &Entry, after: &Entry) -> bool {
    let ownership = |entry: &Entry| entry.owner.as_ref().map(|owner| (owner.uid, owner.gid));
    before.identity == after.identity
        && before.kind == after.kind
        && before.attributes == after.attributes
        && before.link_target == after.link_target
        && ownership(before) == ownership(after)
}
