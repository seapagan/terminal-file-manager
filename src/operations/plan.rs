use crate::{
    app::{IgnoreMode, Pane},
    fs::{Entry, Identity},
};
use std::{io, path::PathBuf};

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
}

impl Plan {
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
        for entry in &operands {
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
        Self::new(operation, operands, destination)
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
        Self::new(operation, operands, destination)
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
        })
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
