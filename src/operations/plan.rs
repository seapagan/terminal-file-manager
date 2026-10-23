use crate::{
    app::{IgnoreMode, Pane},
    fs::{Entry, Identity},
};
use std::{collections::HashSet, io, path::PathBuf};

/// The approved action; transfer destinations refer to a directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Copy,
    Move,
    Mkdir,
    Rename,
    Permissions(u32),
    ReadOnly(bool),
    Trash,
    Delete,
}

/// Approval-time native operands. Workers never consult the current pane.
#[derive(Clone, Debug)]
pub struct Plan {
    pub operation: Operation,
    pub operands: Vec<Entry>,
    pub destination: Option<PathBuf>,
    pub incoming_name: Option<std::ffi::OsString>,
    pub preview: Option<super::Preview>,
    pub(super) completed: HashSet<PathBuf>,
    pub(super) retrying: bool,
    pub(super) source_directories: Vec<DirectoryGuard>,
    pub(super) destination_directory: Option<Box<DirectoryGuard>>,
    retained_directories: Vec<super::Recovery>,
}

impl Plan {
    /// Create-directory intent is independent of the visible cursor and marks.
    pub fn mkdir(destination: PathBuf) -> Self {
        Self {
            operation: Operation::Mkdir,
            operands: Vec::new(),
            destination: Some(destination),
            incoming_name: None,
            preview: None,
            completed: HashSet::new(),
            retrying: false,
            source_directories: Vec::new(),
            destination_directory: None,
            retained_directories: Vec::new(),
        }
    }
    /// Capture the visible working set without filesystem I/O.
    ///
    /// # Errors
    /// Rejects empty, pending, stale marked or unresolved ignore-hidden scope.
    pub fn capture(
        pane: &Pane,
        operation: Operation,
        destination: Option<PathBuf>,
    ) -> io::Result<Self> {
        if pane.pending.is_some() || pane.error.is_some() {
            return Err(invalid(
                "Wait for a successful listing before approving an operation",
            ));
        }
        if pane.ignore == IgnoreMode::Hide && pane.entries.iter().any(|e| e.ignored.is_none()) {
            return Err(invalid(
                "Ignore scope unresolved; refresh before approving an operation",
            ));
        }
        let operands: Vec<_> = pane.eligible().into_iter().cloned().collect();
        validate_marks(pane, &operands)?;
        let mut plan = Self::new(operation, operands, destination)?;
        plan.source_directories.push(DirectoryGuard::cached(pane)?);
        Ok(plan)
    }

    /// Capture explicitly selected native paths on a worker, never the render thread.
    ///
    /// # Errors
    /// Rejects missing identity, empty scope or an absent required destination.
    pub fn paths(
        operation: Operation,
        paths: Vec<PathBuf>,
        destination: Option<PathBuf>,
    ) -> io::Result<Self> {
        let operands = paths.iter().map(|path| crate::fs::inspect(path)).collect();
        let mut plan = Self::new(operation, operands, destination)?;
        for source in &plan.operands {
            if let Some(parent) = source.path.parent() {
                plan.source_directories
                    .push(DirectoryGuard::inspect(parent)?);
            }
        }
        Ok(plan)
    }

    fn new(
        operation: Operation,
        operands: Vec<Entry>,
        destination: Option<PathBuf>,
    ) -> io::Result<Self> {
        if operands.is_empty() || operands.iter().any(|e| e.identity.is_none()) {
            return Err(invalid("No eligible operands with available identity"));
        }
        if matches!(
            operation,
            Operation::Copy | Operation::Move | Operation::Rename | Operation::Mkdir
        ) && destination.is_none()
        {
            return Err(invalid("An explicit destination is required"));
        }
        Ok(Self {
            operation,
            operands,
            destination,
            incoming_name: None,
            preview: None,
            completed: HashSet::new(),
            retrying: false,
            source_directories: Vec::new(),
            destination_directory: None,
            retained_directories: Vec::new(),
        })
    }

    /// Retry incomplete roots from scratch; already committed entries remain excluded.
    ///
    /// # Errors
    /// Unknown outcomes require reconciliation. Fully completed jobs have nothing to retry.
    pub fn retry(&self, record: &super::Record) -> io::Result<Self> {
        if record.unresolved() {
            return Err(invalid("Outcome unknown; reconcile before retry"));
        }
        if self.operation == Operation::Mkdir {
            if record
                .entries
                .iter()
                .any(|entry| entry.outcome == super::Outcome::Completed)
            {
                return Err(invalid("No incomplete directory creation to retry"));
            }
            return Ok(self.clone());
        }
        let mut retry = self.clone();
        retry.preview = None;
        retry.retrying = true;
        retry.retained_directories = record
            .entries
            .iter()
            .filter_map(|entry| {
                (entry.kind == crate::fs::EntryKind::Directory
                    && entry.outcome == super::Outcome::CopiedSourceRetained)
                    .then(|| entry.recovery.as_deref().cloned())
                    .flatten()
            })
            .collect();
        retry.completed.extend(
            record
                .entries
                .iter()
                .filter(|entry| entry.outcome == super::Outcome::Completed)
                .map(|entry| entry.source.clone()),
        );
        retry
            .operands
            .retain(|entry| !retry.completed.contains(&entry.path));
        if retry.operands.is_empty() {
            return Err(invalid("No incomplete operands to retry"));
        }
        Ok(retry)
    }

    pub(super) fn target(&self, source: &Entry) -> Option<PathBuf> {
        self.destination.as_ref().map(|path| {
            if matches!(self.operation, Operation::Copy | Operation::Move) {
                path.join(self.incoming_name.as_deref().unwrap_or(&source.name))
            } else {
                path.clone()
            }
        })
    }

    pub(super) fn refresh_retry(&mut self) -> io::Result<()> {
        if !self.retrying {
            return Ok(());
        }
        for source in &mut self.operands {
            super::safety::revalidate(source, !source.may_recurse())
                .map_err(|error| io::Error::other(error.detail))?;
            *source = crate::fs::inspect(&source.path);
        }
        self.retrying = false;
        Ok(())
    }

    /// Capture an already displayed target directory without I/O on the input thread.
    ///
    /// # Errors
    /// An unresolved directory identity requires a successful refresh first.
    pub fn capture_destination(&mut self, pane: &Pane) -> io::Result<()> {
        self.destination_directory = Some(Box::new(DirectoryGuard::cached(pane)?));
        Ok(())
    }

    /// Explicit target edits invalidate the old target directory's approval stamp.
    pub fn set_destination(&mut self, path: PathBuf) {
        let parent = if matches!(self.operation, Operation::Copy | Operation::Move) {
            Some(path.as_path())
        } else {
            path.parent()
        };
        if self
            .destination_directory
            .as_ref()
            .is_some_and(|guard| Some(guard.raw.as_path()) != parent)
        {
            self.destination_directory = None;
        }
        self.destination = Some(path);
    }

    pub(super) fn validate_directories(&self) -> super::safety::Result<()> {
        for evidence in &self.retained_directories {
            evidence.validate_directory()?;
        }
        self.validate_incoming_name()?;
        for guard in &self.source_directories {
            guard.validate()?;
        }
        if let Some(guard) = &self.destination_directory {
            guard.validate()?;
        }
        Ok(())
    }

    fn validate_incoming_name(&self) -> super::safety::Result<()> {
        if let Some(name) = &self.incoming_name {
            let mut parts = std::path::Path::new(name).components();
            if self.operands.len() != 1
                || !matches!(parts.next(), Some(std::path::Component::Normal(_)))
                || parts.next().is_some()
            {
                return Err(invalid("Incoming name must be one filename for one operand").into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(super) struct DirectoryGuard {
    raw: PathBuf,
    entry: Box<Entry>,
}

impl DirectoryGuard {
    fn cached(pane: &Pane) -> io::Result<Self> {
        let entry = pane.directory.clone().ok_or_else(|| {
            invalid("Directory identity unavailable; refresh before approving mutations")
        })?;
        Ok(Self {
            raw: pane.path.clone(),
            entry: Box::new(entry),
        })
    }
    fn inspect(raw: &std::path::Path) -> io::Result<Self> {
        let entry = crate::fs::inspect(&std::fs::canonicalize(raw)?);
        if entry.kind != crate::fs::EntryKind::Directory {
            return Err(invalid("Mutation parent is not a directory"));
        }
        Ok(Self {
            raw: raw.into(),
            entry: Box::new(entry),
        })
    }
    fn validate(&self) -> super::safety::Result<()> {
        if std::fs::canonicalize(&self.raw)? != self.entry.path {
            return Err(super::safety::changed(
                "Pane namespace changed after approval; refresh and review",
            ));
        }
        super::safety::revalidate(&self.entry, false)
    }
}

pub(super) fn same_object(a: &Identity, b: &Identity) -> bool {
    match (a.object, b.object) {
        (Some(a_object), Some(b_object)) => a_object == b_object && a.created == b.created,
        _ => a == b,
    }
}

pub(super) fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn validate_marks(pane: &Pane, operands: &[Entry]) -> io::Result<()> {
    for entry in operands {
        let identity = entry
            .identity
            .as_ref()
            .ok_or_else(|| invalid("Object identity unavailable"))?;
        if let Some(mark) = pane.marks.get(&entry.path)
            && !same_object(mark, identity)
        {
            return Err(invalid("Marked object changed; review the selection again"));
        }
    }
    Ok(())
}
