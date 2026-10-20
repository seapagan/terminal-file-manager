use super::{Decision, EntryResult, Event, Operation, Options, Outcome, Plan, Record, Stage};
use super::{
    safety::{self, Failure, Parents, Result},
    staging,
};
use crate::fs::{Entry, EntryKind};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

/// Execute a frozen local transfer on a blocking worker. The caller must hold its claims.
/// Cancellation is acknowledged only after this function returns and owned cleanup finishes.
pub fn execute(
    plan: &Plan,
    options: Options,
    cancel: &AtomicBool,
    mut notify: impl FnMut(Event) -> Decision,
) -> Record {
    if let Err(error) = plan.validate_directories() {
        return super::queue::unsuccessful(plan, error.outcome, &error.detail);
    }
    if !matches!(plan.operation, Operation::Copy | Operation::Move) {
        return super::mutations::execute(plan, cancel, &mut notify);
    }
    let mut worker = Worker {
        plan,
        options,
        cancel,
        notify: &mut notify,
        record: Record::default(),
        policies: Vec::new(),
    };
    for entry in &plan.operands {
        if plan.completed.contains(&entry.path) {
            continue;
        }
        let refreshed;
        let entry =
            if plan.retrying && entry.may_recurse() && safety::revalidate(entry, false).is_ok() {
                refreshed = crate::fs::inspect(&entry.path);
                &refreshed
            } else {
                entry
            };
        let destination = plan.target(entry);
        worker.entry(entry, destination.as_deref(), 0);
        if worker.record.unresolved() {
            break;
        }
    }
    worker.record
}

struct Worker<'a, F> {
    plan: &'a Plan,
    options: Options,
    cancel: &'a AtomicBool,
    notify: &'a mut F,
    record: Record,
    policies: Vec<((EntryKind, EntryKind), Decision)>,
}

impl<F: FnMut(Event) -> Decision> Worker<'_, F> {
    fn entry(&mut self, source: &Entry, destination: Option<&Path>, depth: usize) -> bool {
        if self.plan.completed.contains(&source.path) {
            return true;
        }
        let mut result = EntryResult {
            source: source.path.clone(),
            destination: destination.map(Path::to_owned),
            kind: source.kind,
            outcome: Outcome::Completed,
            bytes: 0,
            checksum_verified: false,
            created_directory: false,
            detail: String::new(),
        };
        if let Err(error) = self.perform(source, destination, depth, &mut result) {
            result.outcome = error.outcome;
            result.detail = error.detail;
        }
        let complete = result.outcome == Outcome::Completed;
        self.record.entries.push(result);
        complete
    }

    fn perform(
        &mut self,
        source: &Entry,
        destination: Option<&Path>,
        depth: usize,
        result: &mut EntryResult,
    ) -> Result<()> {
        self.check_cancel()?;
        prepare_source(source, depth)?;
        let destination =
            destination.ok_or_else(|| super::plan::invalid("Destination required"))?;
        safety::reject_overlap(source, destination)?;
        if !matches!(self.plan.operation, Operation::Copy | Operation::Move) {
            return Err(
                super::plan::invalid("This executor accepts copy and move transfers").into(),
            );
        }
        let Some((destination, previous)) = self.resolve(source, destination)? else {
            result.outcome = Outcome::Skipped;
            return Ok(());
        };
        result.destination = Some(destination.clone());
        if source.may_recurse() {
            return self.directory(source, &destination, previous.as_ref(), depth, result);
        }
        self.leaf(source, &destination, previous.as_ref(), result)?;
        Ok(())
    }

    fn resolve(
        &mut self,
        source: &Entry,
        destination: &Path,
    ) -> Result<Option<(PathBuf, Option<Entry>)>> {
        let Some(previous) = safety::existing(destination)? else {
            return Ok(Some((destination.into(), None)));
        };
        if source.may_recurse() && previous.may_recurse() {
            return Ok(Some((destination.into(), Some(previous))));
        }
        let newer_available = newer_available(source, &previous);
        let decision = self.conflict_decision(source, &previous, newer_available);
        self.check_cancel()?;
        match decision {
            Decision::Replace => Ok(Some((destination.into(), Some(previous)))),
            Decision::Newer if newer_available && source.modified > previous.modified => {
                Ok(Some((destination.into(), Some(previous))))
            }
            Decision::KeepBoth => Ok(Some((keep_both(destination)?, None))),
            Decision::Cancel => {
                self.cancel.store(true, Ordering::Relaxed);
                self.check_cancel()?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn conflict_decision(
        &mut self,
        source: &Entry,
        previous: &Entry,
        newer_available: bool,
    ) -> Decision {
        let class = (source.kind, previous.kind);
        let decision = self
            .policies
            .iter()
            .find(|(key, _)| *key == class)
            .map(|(_, decision)| decision.clone())
            .unwrap_or_else(|| {
                (self.notify)(Event::Conflict {
                    source: Box::new(source.clone()),
                    destination: Box::new(previous.clone()),
                    newer_available,
                })
            });
        if let Decision::All(decision) = decision {
            self.policies.push((class, *decision.clone()));
            *decision
        } else {
            decision
        }
    }

    fn directory(
        &mut self,
        source: &Entry,
        destination: &Path,
        previous: Option<&Entry>,
        depth: usize,
        result: &mut EntryResult,
    ) -> Result<()> {
        if previous.is_some_and(|entry| !entry.may_recurse()) {
            return Err(super::plan::invalid(
                "Atomic file/directory replacement unsupported; choose Keep both or Skip",
            )
            .into());
        }
        let source_parents = Parents::capture(&source.path)?;
        let target_parents = Parents::capture(destination)?;
        safety::destination_matches(destination, previous)?;
        if previous.is_none() {
            fs::create_dir(destination)?;
            result.created_directory = true;
        }
        let target = crate::fs::inspect(destination);
        let complete = self.children(source, &target, depth, [&source_parents, &target_parents])?;
        if !complete {
            result.outcome = Outcome::CopiedSourceRetained;
            return Ok(());
        }
        self.finish_directory(source, &target, [&source_parents, &target_parents])
    }

    fn children(
        &mut self,
        source: &Entry,
        target: &Entry,
        depth: usize,
        guards: [&Parents; 2],
    ) -> Result<bool> {
        let mut complete = true;
        let entries: std::io::Result<Vec<_>> = fs::read_dir(&source.path)?.collect();
        for item in entries? {
            self.check_cancel()?;
            guards[0].validate()?;
            safety::revalidate(source, false)?;
            guards[1].validate()?;
            safety::revalidate(target, false)?;
            let entry = crate::fs::inspect(&item.path());
            complete &= self.entry(&entry, Some(&target.path.join(&entry.name)), depth + 1);
            if self.record.unresolved() {
                return Err(Failure {
                    outcome: Outcome::OutcomeUnknown,
                    detail: "Child outcome unresolved".into(),
                });
            }
        }
        Ok(complete)
    }

    fn finish_directory(
        &mut self,
        source: &Entry,
        target: &Entry,
        guards: [&Parents; 2],
    ) -> Result<()> {
        guards[1].validate()?;
        safety::revalidate(target, false)?;
        fs::set_permissions(
            &target.path,
            fs::symlink_metadata(&source.path)?.permissions(),
        )?;
        if self.plan.operation == Operation::Move {
            self.check_cancel()?;
            guards[0].validate()?;
            safety::revalidate(source, false)?;
            fs::remove_dir(&source.path)?;
        }
        Ok(())
    }

    fn leaf(
        &mut self,
        source: &Entry,
        destination: &Path,
        previous: Option<&Entry>,
        result: &mut EntryResult,
    ) -> Result<()> {
        if !matches!(source.kind, EntryKind::File | EntryKind::Link) {
            return Err(super::plan::invalid(
                "Unsupported special/reparse entry; no stream opened",
            )
            .into());
        }
        if previous.is_some_and(|entry| entry.may_recurse()) {
            return Err(super::plan::invalid(
                "Atomic directory/file replacement unsupported; choose Keep both or Skip",
            )
            .into());
        }
        let guards = [
            Parents::capture(&source.path)?,
            Parents::capture(destination)?,
        ];
        let mut stage = staging::OwnedStage::create(
            destination,
            (source.kind == EntryKind::Link).then_some(source),
        )?;
        let transfer = self.staged(source, destination, previous, &mut stage, result, &guards);
        let committed = stage.finish(transfer)?;
        if self.plan.operation == Operation::Move {
            let removal = self.remove_source(source, &committed, &guards);
            if let Err(error) = removal {
                result.outcome = if error.outcome == Outcome::OutcomeUnknown {
                    Outcome::OutcomeUnknown
                } else {
                    Outcome::CopiedSourceRetained
                };
                result.detail = error.detail;
            }
        }
        Ok(())
    }

    fn staged(
        &mut self,
        source: &Entry,
        destination: &Path,
        previous: Option<&Entry>,
        stage: &mut staging::OwnedStage,
        result: &mut EntryResult,
        guards: &[Parents; 2],
    ) -> Result<Entry> {
        self.stage_payload(source, destination, stage, result)?;
        stage.seal()?;
        (self.notify)(Event::Progress {
            source: source.path.clone(),
            destination: destination.into(),
            stage: Stage::Verified,
            bytes: result.bytes,
        });
        self.check_cancel()?;
        validate_guards(guards)?;
        safety::revalidate(source, true)?;
        safety::destination_matches(destination, previous)?;
        stage.commit(destination, previous.is_some())?;
        let committed = stage.committed(destination)?;
        (self.notify)(Event::Progress {
            source: source.path.clone(),
            destination: destination.into(),
            stage: Stage::Committed,
            bytes: result.bytes,
        });
        if let Err(error) = safety::revalidate(&committed, true) {
            result.outcome = Outcome::CopiedSourceRetained;
            return Err(Failure {
                outcome: Outcome::CopiedSourceRetained,
                detail: format!(
                    "Destination changed after commit; source retained: {}",
                    error.detail
                ),
            });
        }
        Ok(committed)
    }

    fn stage_payload(
        &mut self,
        source: &Entry,
        destination: &Path,
        stage: &mut staging::OwnedStage,
        result: &mut EntryResult,
    ) -> Result<()> {
        if source.kind == EntryKind::File {
            result.bytes = super::transfer::copy(
                source,
                stage.file.as_mut().expect("regular stage"),
                self.options,
                self.cancel,
                &mut |bytes| {
                    (self.notify)(Event::Progress {
                        source: source.path.clone(),
                        destination: destination.into(),
                        stage: Stage::Reading,
                        bytes,
                    });
                },
            )?;
            result.checksum_verified = self.options.checksum;
        }
        Ok(())
    }

    fn remove_source(
        &mut self,
        source: &Entry,
        destination: &Entry,
        guards: &[Parents; 2],
    ) -> Result<()> {
        self.check_cancel()?;
        validate_guards(guards)?;
        safety::revalidate(source, true)?;
        safety::revalidate(destination, true)?;
        (self.notify)(Event::Progress {
            source: source.path.clone(),
            destination: destination.path.clone(),
            stage: Stage::Removing,
            bytes: source.size.unwrap_or(0),
        });
        self.check_cancel()?;
        validate_guards(guards)?;
        safety::revalidate(source, true)?;
        safety::revalidate(destination, true)?;
        safety::acknowledged_remove(source, staging::remove_leaf)?;
        Ok(())
    }

    fn check_cancel(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Job cancelled; completed destinations retained",
            )
            .into())
        } else {
            Ok(())
        }
    }
}

fn keep_both(path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| super::plan::invalid("Missing destination name"))?;
    for number in 1..=10_000 {
        let mut name = name.to_owned();
        name.push(format!(" (copy {number})"));
        let candidate = path.with_file_name(name);
        if safety::existing(&candidate)?.is_none() {
            return Ok(candidate);
        }
    }
    Err(super::plan::invalid("Keep-both namespace exhausted").into())
}

fn validate_guards(guards: &[Parents; 2]) -> Result<()> {
    for guard in guards {
        guard.validate()?;
    }
    Ok(())
}

fn prepare_source(source: &Entry, depth: usize) -> Result<()> {
    if depth > 128 {
        return Err(super::plan::invalid("Maximum recursion depth reached").into());
    }
    Parents::capture(&source.path)?.validate()?;
    safety::revalidate(source, true)
}

fn newer_available(source: &Entry, destination: &Entry) -> bool {
    source.modified.is_some() && destination.modified.is_some() && source.kind == destination.kind
}
