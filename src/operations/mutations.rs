use super::{
    Decision, EntryResult, Event, Operation, Outcome, Plan, Record, Stage,
    safety::{self, Parents, Result},
    staging,
};
use crate::fs::{Entry, EntryKind};
use std::{fs, io, sync::atomic::AtomicBool};

pub(super) fn execute(
    plan: &Plan,
    cancel: &AtomicBool,
    notify: &mut impl FnMut(Event) -> Decision,
) -> Record {
    if matches!(plan.operation, Operation::Trash | Operation::Delete) && plan.preview.is_none() {
        let mut approved = plan.clone();
        match super::Preview::capture(plan, cancel) {
            Ok(preview) => approved.preview = Some(preview),
            Err(error) => return super::queue::unsuccessful(plan, error.outcome, &error.detail),
        }
        return execute(&approved, cancel, notify);
    }
    let mut record = Record::default();
    if let Some(preview) = &plan.preview
        && let Err(error) = preview.validate(plan, cancel)
    {
        return super::queue::unsuccessful(plan, error.outcome, &error.detail);
    }
    if plan.operation == Operation::Mkdir {
        if let Some(destination) = &plan.destination {
            let source = crate::fs::inspect(destination.parent().unwrap_or(destination));
            apply(&source, plan, cancel, notify, &mut record, 0);
        }
    } else {
        for source in &plan.operands {
            apply(source, plan, cancel, notify, &mut record, 0);
            if record.unresolved() {
                break;
            }
        }
    }
    record
}

fn apply(
    source: &Entry,
    plan: &Plan,
    cancel: &AtomicBool,
    notify: &mut impl FnMut(Event) -> Decision,
    record: &mut Record,
    depth: usize,
) -> bool {
    let mut result = EntryResult {
        recovery: None,
        source: source.path.clone(),
        destination: plan.destination.clone(),
        kind: source.kind,
        outcome: Outcome::Completed,
        bytes: 0,
        checksum_verified: false,
        created_directory: source.may_recurse()
            && matches!(plan.operation, Operation::Delete | Operation::Trash),
        detail: String::new(),
    };
    let changed = perform(source, plan, cancel, notify, record, depth, &mut result);
    if let Err(error) = changed {
        result.outcome = error.outcome;
        result.detail = error.detail;
    }
    let complete = result.outcome == Outcome::Completed;
    record.entries.push(result);
    complete
}

fn perform(
    source: &Entry,
    plan: &Plan,
    cancel: &AtomicBool,
    notify: &mut impl FnMut(Event) -> Decision,
    record: &mut Record,
    depth: usize,
    result: &mut EntryResult,
) -> Result<()> {
    crate::fs::check_cancelled(cancel)?;
    if depth > 128 {
        return Err(io::Error::other("Maximum mutation depth reached").into());
    }
    if plan.operation == Operation::Mkdir {
        return mkdir(plan, result);
    }
    let parents = Parents::capture(&source.path)?;
    safety::revalidate(source, true)?;
    match plan.operation {
        Operation::Rename => rename(source, plan, result),
        Operation::Permissions(mode) => attributes(source, Some(mode), None),
        Operation::ReadOnly(value) => attributes(source, None, Some(value)),
        Operation::Delete => delete(source, plan, cancel, notify, record, depth, &parents),
        Operation::Trash => recycle(source, result, record),
        _ => Err(io::Error::other("Unsupported mutation kind").into()),
    }
}

fn mkdir(plan: &Plan, result: &mut EntryResult) -> Result<()> {
    let destination = plan
        .destination
        .as_ref()
        .ok_or_else(|| io::Error::other("Directory name required"))?;
    let parents = Parents::capture(destination)?;
    parents.validate()?;
    fs::create_dir(destination)?;
    result.kind = EntryKind::Directory;
    result.created_directory = true;
    Ok(())
}

fn rename(source: &Entry, plan: &Plan, result: &mut EntryResult) -> Result<()> {
    let destination = rename_destination(plan)?;
    safety::reject_overlap(source, destination)?;
    if safety::existing(destination)?.is_some() {
        result.outcome = Outcome::Skipped;
        result.detail = "Rename destination exists; choose a different name".into();
        return Ok(());
    }
    let parents = Parents::capture(destination)?;
    parents.validate()?;
    safety::revalidate(source, true)?;
    #[cfg(unix)]
    staging::rename_new(&source.path, destination)?;
    #[cfg(windows)]
    fs::rename(&source.path, destination)?;
    Ok(())
}

fn rename_destination(plan: &Plan) -> io::Result<&std::path::Path> {
    if plan.operands.len() != 1 {
        return Err(io::Error::other(
            "Rename requires exactly one selected entry",
        ));
    }
    plan.destination
        .as_deref()
        .ok_or_else(|| io::Error::other("New name required"))
}

fn attributes(source: &Entry, mode: Option<u32>, readonly: Option<bool>) -> Result<()> {
    if !matches!(source.kind, EntryKind::File | EntryKind::Directory) {
        return Err(
            io::Error::other("Attribute changes do not follow links/reparse points").into(),
        );
    }
    let file = open_attributes(source)?;
    safety::opened_object(source, &file)?;
    let mut permissions = file.metadata()?.permissions();
    apply_permissions(&mut permissions, mode, readonly)?;
    safety::revalidate(source, true)?;
    file.set_permissions(permissions)?;
    Ok(())
}

fn apply_permissions(
    permissions: &mut fs::Permissions,
    mode: Option<u32>,
    readonly: Option<bool>,
) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = mode.filter(|mode| *mode <= 0o7777).ok_or_else(|| {
            io::Error::other("Enter Unix mode in 0000..7777; readonly flags are Windows-only")
        })?;
        permissions.set_mode(mode);
        let _ = readonly;
    }
    #[cfg(windows)]
    {
        let readonly = readonly
            .ok_or_else(|| io::Error::other("Unix mode unsupported on Windows; edit Read-only"))?;
        permissions.set_readonly(readonly);
        let _ = mode;
    }
    Ok(())
}

fn open_attributes(source: &Entry) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0220_0000).access_mode(0x0000_0180);
    }
    options.open(&source.path)
}

fn delete(
    source: &Entry,
    plan: &Plan,
    cancel: &AtomicBool,
    notify: &mut impl FnMut(Event) -> Decision,
    record: &mut Record,
    depth: usize,
    parents: &Parents,
) -> Result<()> {
    if source.may_recurse() {
        let children: io::Result<Vec<_>> = fs::read_dir(&source.path)?.collect();
        for child in children? {
            crate::fs::check_cancelled(cancel)?;
            parents.validate()?;
            safety::revalidate(source, false)?;
            if !apply(
                &approved_child(plan, &child.path())?,
                plan,
                cancel,
                notify,
                record,
                depth + 1,
            ) {
                return Err(safety::changed("Incomplete deletion; parent retained"));
            }
        }
    }
    remove(source, cancel, notify, parents)
}

fn approved_child(plan: &Plan, path: &std::path::Path) -> Result<Entry> {
    plan.preview.as_ref().map_or_else(
        || Ok(crate::fs::inspect(path)),
        |preview| preview.approved(path),
    )
}

fn remove(
    source: &Entry,
    cancel: &AtomicBool,
    notify: &mut impl FnMut(Event) -> Decision,
    parents: &Parents,
) -> Result<()> {
    crate::fs::check_cancelled(cancel)?;
    (notify)(Event::Progress {
        source: source.path.clone(),
        destination: source.path.clone(),
        stage: Stage::Removing,
        bytes: 0,
    });
    crate::fs::check_cancelled(cancel)?;
    parents.validate()?;
    safety::revalidate(source, !source.may_recurse())?;
    safety::acknowledged_remove(source, |path| {
        if matches!(source.kind, EntryKind::Directory | EntryKind::Junction) {
            fs::remove_dir(path)
        } else if source.kind == EntryKind::ReparsePoint {
            Err(io::Error::other(
                "Unsupported reparse object; deletion refused",
            ))
        } else {
            staging::remove_leaf(path)
        }
    })
}

fn recycle(source: &Entry, result: &mut EntryResult, record: &mut Record) -> Result<()> {
    let receipt = super::trash::put(source).map_err(|error| {
        if safety::revalidate(source, false).is_ok() {
            error.into()
        } else {
            safety::Failure {
                outcome: Outcome::OutcomeUnknown,
                detail: format!("Trash outcome unknown; source may already be recycled: {error}"),
            }
        }
    })?;
    result.destination = Some(receipt.path.clone());
    result.detail = format!("Recycled; recovery location {}", receipt.path.display());
    let moved = crate::fs::inspect(&receipt.path);
    if moved.kind != source.kind || moved.link_target != source.link_target {
        return Err(safety::Failure {
            outcome: Outcome::OutcomeUnknown,
            detail: format!(
                "Native Trash receipt type/link unverified: {}",
                receipt.path.display()
            ),
        });
    }
    let mut children = Vec::new();
    scan_recycled(&moved, 0, &mut children).map_err(|error| safety::Failure {
        outcome: Outcome::OutcomeUnknown,
        detail: format!(
            "Recycled at {}; recursive result counts unavailable: {error}",
            receipt.path.display()
        ),
    })?;
    for child in children {
        let relative = child
            .path
            .strip_prefix(&receipt.path)
            .map_err(io::Error::other)?;
        record.entries.push(EntryResult {
            recovery: None,
            source: source.path.join(relative),
            destination: Some(child.path.clone()),
            kind: child.kind,
            outcome: Outcome::Completed,
            bytes: 0,
            checksum_verified: false,
            created_directory: child.may_recurse(),
            detail: result.detail.clone(),
        });
    }
    Ok(())
}

fn scan_recycled(entry: &Entry, depth: usize, entries: &mut Vec<Entry>) -> io::Result<()> {
    if depth > 128 {
        return Err(io::Error::other("Maximum Trash inspection depth reached"));
    }
    if entry.may_recurse() {
        for child in fs::read_dir(&entry.path)? {
            let entry = crate::fs::inspect(&child?.path());
            if entry.identity.is_none() {
                return Err(io::Error::other("Trashed descendant identity unavailable"));
            }
            scan_recycled(&entry, depth + 1, entries)?;
            entries.push(entry);
        }
    }
    Ok(())
}
