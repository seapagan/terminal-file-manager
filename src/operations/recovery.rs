use super::{
    Operation, Outcome, Plan, Record,
    safety::{self, Parents, Result},
    staging,
};
use crate::fs::Entry;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

/// Exact ownership evidence survives uncertain cleanup; display text is never parsed as a path.
#[derive(Clone, Debug)]
pub struct Recovery {
    pub(super) source: Entry,
    pub(super) destination: PathBuf,
    pub(super) previous: Option<Entry>,
    pub(super) committed: Option<Entry>,
    pub(super) staging: Option<Staging>,
    pub(super) guards: [Parents; 2],
}

#[derive(Clone, Debug)]
pub(super) struct Staging {
    pub root: Entry,
    pub payload: Entry,
}

impl Recovery {
    pub fn staging_path(&self) -> Option<&Path> {
        self.staging.as_ref().map(|stage| stage.root.path.as_path())
    }

    pub(super) fn directory(source: &Entry, target: &Entry, guards: &[Parents; 2]) -> Self {
        Self {
            source: source.clone(),
            destination: target.path.clone(),
            previous: None,
            committed: Some(target.clone()),
            staging: None,
            guards: guards.clone(),
        }
    }

    fn cleanup(&self) -> Result<()> {
        let Some(stage) = &self.staging else {
            return Ok(());
        };
        if safety::existing(&stage.root.path)?.is_none() {
            return Ok(());
        }
        safety::revalidate(&stage.root, false)?;
        if safety::existing(&stage.payload.path)?.is_some() {
            safety::owned_object(&stage.payload)?;
            staging::remove_leaf(&stage.payload.path)?;
        }
        fs::remove_dir(&stage.root.path)?;
        Ok(())
    }

    fn destination_evidence(&self) -> Result<Entry> {
        if let Some(committed) = &self.committed {
            return Ok(committed.clone());
        }
        if let Some(stage) = &self.staging {
            let mut expected = stage.payload.clone();
            expected.path = self.destination.clone();
            return Ok(expected);
        }
        Err(safety::changed(
            "Committed destination identity unavailable",
        ))
    }

    fn inspect(&self, operation: Operation) -> Result<(Outcome, &'static str)> {
        for guard in &self.guards {
            guard.validate()?;
        }
        let destination = self.destination_evidence()?;
        if self.staging.is_none() {
            return self.directory_outcome(&destination);
        }
        if safety::revalidate(&destination, true).is_ok() {
            let outcome = self.committed_outcome(operation)?;
            self.cleanup()?;
            safety::revalidate(&destination, true)?;
            return Ok((
                outcome,
                "Reconciled committed destination; no source removal reissued",
            ));
        }
        self.retained_outcome()
    }

    fn directory_outcome(&self, destination: &Entry) -> Result<(Outcome, &'static str)> {
        safety::revalidate(&self.source, false)?;
        safety::revalidate(destination, false)?;
        Ok((
            Outcome::CopiedSourceRetained,
            "Reconciled child outcomes; source directory retained; retry from scratch",
        ))
    }

    fn retained_outcome(&self) -> Result<(Outcome, &'static str)> {
        safety::revalidate(&self.source, true)?;
        let unchanged =
            safety::destination_matches(&self.destination, self.previous.as_ref()).is_ok();
        self.cleanup()?;
        Ok((
            if unchanged {
                Outcome::Failed
            } else {
                Outcome::Changed
            },
            "Reconciled retained source; cleaned only owned staging; retry requires fresh checks",
        ))
    }

    fn committed_outcome(&self, operation: Operation) -> Result<Outcome> {
        if operation != Operation::Move {
            return Ok(Outcome::Completed);
        }
        if safety::revalidate(&self.source, true).is_ok() {
            return Ok(Outcome::CopiedSourceRetained);
        }
        Ok(if safety::existing(&self.source.path)?.is_none() {
            Outcome::Completed
        } else {
            Outcome::Changed
        })
    }
}

/// Reinspect a stopped job under its retained claims; never reissue its mutations.
/// Missing or changed ownership evidence remains unknown and keeps claims quarantined.
pub fn reconcile(plan: &Plan, mut record: Record, cancel: &AtomicBool) -> Record {
    let mut pending: BTreeSet<_> = record
        .entries
        .iter()
        .filter(|entry| entry.outcome == Outcome::OutcomeUnknown)
        .map(|entry| entry.source.clone())
        .collect();
    for index in 0..record.entries.len() {
        let entry = &record.entries[index];
        if entry.outcome != Outcome::OutcomeUnknown {
            continue;
        }
        let Some(evidence) = &entry.recovery else {
            continue;
        };
        if waiting_on_children(evidence, &pending) {
            continue;
        }
        let result = crate::fs::check_cancelled(cancel)
            .map_err(Into::into)
            .and_then(|()| evidence.inspect(plan.operation));
        let entry = &mut record.entries[index];
        match result {
            Ok((outcome, detail)) => {
                entry.outcome = outcome;
                annotate(entry, detail);
                pending.remove(&entry.source);
            }
            Err(error) => {
                annotate(
                    entry,
                    &format!("Reconciliation remains unknown: {}", error.detail),
                );
            }
        }
    }
    record
}

fn annotate(entry: &mut super::EntryResult, detail: &str) {
    if !entry.detail.ends_with(detail) {
        entry.detail = format!("{}; {detail}", entry.detail);
    }
}

fn waiting_on_children(evidence: &Recovery, pending: &BTreeSet<PathBuf>) -> bool {
    evidence.staging.is_none()
        && pending
            .range(evidence.source.path.clone()..)
            .nth(1)
            .is_some_and(|path| path.starts_with(&evidence.source.path))
}
