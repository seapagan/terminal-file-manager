use crate::fs::{Entry, EntryKind};
use std::path::PathBuf;

/// Stronger verification is opt-in; metadata never proves byte equality.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub checksum: bool,
}

/// Stages describe observed lifecycle transitions, not queued intentions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Reading,
    Verified,
    Committed,
    Removing,
}

/// Conflict responses are scoped by the worker to one job and conflict class.
#[derive(Clone, Debug)]
pub enum Decision {
    Skip,
    Replace,
    KeepBoth,
    Newer,
    Cancel,
    All(Box<Decision>),
}

/// Worker events contain native identities; rendering must sanitize display text.
#[derive(Clone, Debug)]
pub enum Event {
    OwnedStage {
        root: Box<Entry>,
        payload: Box<Entry>,
    },
    Progress {
        source: PathBuf,
        destination: PathBuf,
        stage: Stage,
        bytes: u64,
    },
    Conflict {
        source: Box<Entry>,
        destination: Box<Entry>,
        newer_available: bool,
    },
}

/// Source retention and unknown outcomes are distinct from completed moves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Completed,
    Skipped,
    Failed,
    Changed,
    Cancelled,
    CopiedSourceRetained,
    OutcomeUnknown,
}

/// One actual recursive entry outcome; partial directories are never completed.
#[derive(Clone, Debug)]
pub struct EntryResult {
    pub recovery: Option<Box<super::Recovery>>,
    pub source: PathBuf,
    pub destination: Option<PathBuf>,
    pub kind: EntryKind,
    pub outcome: Outcome,
    pub bytes: u64,
    pub checksum_verified: bool,
    pub created_directory: bool,
    pub detail: String,
}

/// In-session evidence retained after progress and toasts expire.
#[derive(Clone, Debug, Default)]
pub struct Record {
    pub entries: Vec<EntryResult>,
}

impl Record {
    /// Observed transferred bytes, including copied-but-source-retained data.
    pub fn bytes(&self) -> u64 {
        self.entries.iter().map(|e| e.bytes).sum()
    }
    /// Completed non-directory entries, excluding retained move sources.
    pub fn completed_files(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.kind != EntryKind::Directory && e.outcome == Outcome::Completed)
            .count()
    }
    /// Only newly created directories whose whole subtree completed.
    pub fn completed_directories(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.created_directory && e.outcome == Outcome::Completed)
            .count()
    }
    /// Unresolved outcomes retain locks and block automatic retry and safe exit.
    pub fn unresolved(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.outcome == Outcome::OutcomeUnknown)
    }
}
