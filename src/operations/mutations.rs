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
    if let Err(error) = validate_preview(plan, cancel) {
        return super::queue::unsuccessful(plan, error.outcome, &error.detail);
    }
    if plan.operation.creates_entry() {
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

fn validate_preview(plan: &Plan, cancel: &AtomicBool) -> Result<()> {
    // Linux Trash validates each immutable approved root immediately before native rename.
    if cfg!(target_os = "linux") && plan.operation == Operation::Trash {
        return Ok(());
    }
    if let Some(preview) = &plan.preview {
        preview.validate(plan, cancel)?;
    }
    Ok(())
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
        trash: None,
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
    if plan.operation.creates_entry() {
        return create_entry(plan, result);
    }
    let parents = Parents::capture(&source.path)?;
    safety::revalidate(source, true)?;
    match plan.operation {
        Operation::Rename => rename(source, plan, result, notify, cancel, &parents),
        Operation::Permissions(mode) => attributes(source, Some(mode), None, &parents),
        Operation::ReadOnly(value) => attributes(source, None, Some(value), &parents),
        Operation::Delete => delete(source, plan, cancel, notify, record, depth, &parents),
        Operation::Trash => recycle(source, plan, cancel, result, notify),
        _ => Err(io::Error::other("Unsupported mutation kind").into()),
    }
}

fn create_entry(plan: &Plan, result: &mut EntryResult) -> Result<()> {
    let destination = plan
        .destination
        .as_ref()
        .ok_or_else(|| io::Error::other("Creation destination required"))?;
    let parents = Parents::capture(destination)?;
    parents.validate()?;
    if plan.operation == Operation::CreateFile {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        result.kind = EntryKind::File;
    } else {
        fs::create_dir(destination)?;
        result.kind = EntryKind::Directory;
        result.created_directory = true;
    }
    Ok(())
}

fn rename(
    source: &Entry,
    plan: &Plan,
    result: &mut EntryResult,
    notify: &mut impl FnMut(Event) -> Decision,
    cancel: &AtomicBool,
    source_parents: &Parents,
) -> Result<()> {
    let destination = rename_destination(plan)?;
    if !rename_available(source, destination)? {
        result.outcome = Outcome::Skipped;
        result.detail = "Rename destination exists; choose a different name".into();
        return Ok(());
    }
    let parents = Parents::capture(destination)?;
    (notify)(Event::Progress {
        source: source.path.clone(),
        destination: destination.into(),
        stage: Stage::Verified,
        bytes: 0,
    });
    crate::fs::check_cancelled(cancel)?;
    source_parents.validate()?;
    parents.validate()?;
    safety::revalidate(source, true)?;
    commit_rename(source, destination, result)
}

fn rename_available(source: &Entry, destination: &std::path::Path) -> Result<bool> {
    if safety::same_entry_rename(source, destination)? {
        return Ok(true);
    }
    safety::reject_overlap(source, destination)?;
    Ok(safety::existing(destination)?.is_none())
}

fn commit_rename(
    source: &Entry,
    destination: &std::path::Path,
    result: &mut EntryResult,
) -> Result<()> {
    if let Err(error) = staging::rename_new(&source.path, destination) {
        if safety::revalidate(source, true).is_err() {
            return Err(safety::Failure {
                outcome: Outcome::OutcomeUnknown,
                detail: format!(
                    "Rename unacknowledged; selected object requires reconciliation: {error}"
                ),
            });
        }
        if error.kind() == io::ErrorKind::AlreadyExists {
            result.outcome = Outcome::Skipped;
            result.detail = "Native rename refused an existing destination; source retained".into();
            return Ok(());
        }
        return Err(error.into());
    }
    let mut moved = source.clone();
    moved.path = destination.into();
    safety::revalidate(&moved, true).map_err(|error| safety::Failure {
        outcome: Outcome::OutcomeUnknown,
        detail: format!("Renamed object identity unverified: {}", error.detail),
    })?;
    renamed_spelling(destination).map_err(|error| safety::Failure {
        outcome: Outcome::OutcomeUnknown,
        detail: format!(
            "Native rename returned success but filename spelling is unverified: {error}"
        ),
    })
}

fn renamed_spelling(destination: &std::path::Path) -> io::Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other("Rename parent required"))?;
    for entry in fs::read_dir(parent)? {
        if Some(entry?.file_name().as_os_str()) == destination.file_name() {
            return Ok(());
        }
    }
    Err(io::Error::other("Requested native name not found"))
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

fn attributes(
    source: &Entry,
    mode: Option<u32>,
    readonly: Option<bool>,
    parents: &Parents,
) -> Result<()> {
    if !matches!(source.kind, EntryKind::File | EntryKind::Directory) {
        return Err(
            io::Error::other("Attribute changes do not follow links/reparse points").into(),
        );
    }
    let file = open_attributes(source)?;
    safety::opened_object(source, &file)?;
    let mut permissions = file.metadata()?.permissions();
    apply_permissions(&mut permissions, mode, readonly)?;
    parents.validate()?;
    safety::revalidate(source, true)?;
    set_attributes(&file, permissions)?;
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

pub(super) fn open_attributes(source: &Entry) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let flags = nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK;
        #[cfg(target_os = "linux")]
        let flags = flags | nix::libc::O_PATH;
        options.custom_flags(flags);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0220_0000).access_mode(0x0000_0180);
    }
    options.open(&source.path)
}

pub(super) fn set_attributes(file: &fs::File, permissions: fs::Permissions) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        // O_PATH pins the object without read access; procfs resolves the held fd, not its old name.
        fs::set_permissions(format!("/proc/self/fd/{}", file.as_raw_fd()), permissions)
    }
    #[cfg(not(target_os = "linux"))]
    file.set_permissions(permissions)
}

#[cfg(all(test, unix))]
mod attribute_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn attribute_descriptor_never_retargets_a_replacement_name_or_link() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("selected");
        fs::write(&path, "selected bytes").unwrap();
        let mode = if cfg!(target_os = "linux") {
            0o000
        } else {
            0o400
        };
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        let selected = crate::fs::inspect(&path);
        let file = open_attributes(&selected).unwrap();
        safety::opened_object(&selected, &file).unwrap();
        let retained = root.path().join("retained");
        fs::rename(&path, &retained).unwrap();
        let outside = root.path().join("outside");
        fs::write(&outside, "sentinel").unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o640)).unwrap();
        std::os::unix::fs::symlink(&outside, &path).unwrap();
        if let Ok(replacement) = open_attributes(&selected) {
            assert!(safety::opened_object(&selected, &replacement).is_err());
        }
        assert!(safety::revalidate(&selected, true).is_err());
        set_attributes(&file, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            fs::metadata(&retained).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&outside).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(fs::read(retained).unwrap(), b"selected bytes");
        assert_eq!(fs::read(outside).unwrap(), b"sentinel");
    }
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

fn recycle(
    source: &Entry,
    plan: &Plan,
    cancel: &AtomicBool,
    result: &mut EntryResult,
    notify: &mut impl FnMut(Event) -> Decision,
) -> Result<()> {
    let approval = plan
        .preview
        .as_ref()
        .ok_or_else(|| io::Error::other("Validated Trash inventory required"))?;
    let (files, directories) = approval.inventory(&source.path);
    crate::fs::check_cancelled(cancel)?;
    (notify)(Event::Progress {
        source: source.path.clone(),
        destination: source.path.clone(),
        stage: Stage::Removing,
        bytes: 0,
    });
    let receipt = super::trash::put(source, cancel, approval)?;
    result.destination = receipt.path().map(std::path::Path::to_owned);
    result.detail = format!(
        "{}; validated before move: {files} files, {directories} directories",
        receipt.description()
    );
    result.trash = Some(super::TrashResult {
        receipt,
        validated_files: files,
        validated_directories: directories,
    });
    Ok(())
}
