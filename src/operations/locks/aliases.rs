use super::*;
use crate::{
    fs::Entry,
    operations::{Operation, mounts::Mounts, safety},
};

pub(super) fn capture(entry: &Entry) -> io::Result<HashSet<(u64, u64)>> {
    safety::revalidate(entry, false).map_err(|error| io::Error::other(error.detail))?;
    if !entry.may_recurse() {
        return entry
            .identity
            .as_ref()
            .and_then(|id| id.object)
            .map(|object| HashSet::from([object]))
            .ok_or_else(|| invalid("Native alias identity unavailable"));
    }
    directory(entry)
}

fn directory(entry: &Entry) -> io::Result<HashSet<(u64, u64)>> {
    let guard =
        safety::Parents::capture(&entry.path).map_err(|error| io::Error::other(error.detail))?;
    #[cfg(target_os = "linux")]
    let namespace = std::fs::read("/proc/self/mountinfo")?;
    let mounts = Mounts::capture(Operation::Delete)?;
    let mut objects = HashSet::new();
    scan(&crate::fs::inspect(&entry.path), &mut objects, &mounts, 0)?;
    guard
        .validate()
        .map_err(|error| io::Error::other(error.detail))?;
    #[cfg(target_os = "linux")]
    if std::fs::read("/proc/self/mountinfo")? != namespace {
        return Err(invalid("Mount namespace changed during alias inventory"));
    }
    Ok(objects)
}

fn scan(
    entry: &Entry,
    objects: &mut HashSet<(u64, u64)>,
    mounts: &Mounts,
    depth: usize,
) -> io::Result<()> {
    if depth > 128 {
        return Err(invalid("Alias inventory exceeds the traversal boundary"));
    }
    mounts
        .reject(entry)
        .map_err(|error| io::Error::other(error.detail))?;
    let object = entry
        .identity
        .as_ref()
        .and_then(|id| id.object)
        .ok_or_else(|| invalid("Native alias identity unavailable"))?;
    if !objects.insert(object) && entry.may_recurse() {
        return Err(invalid(
            "Repeated directory identity; alias inventory uncertain",
        ));
    }
    if entry.may_recurse() {
        children(entry, objects, mounts, depth)?;
    }
    safety::revalidate(entry, true).map_err(|error| io::Error::other(error.detail))
}

fn children(
    entry: &Entry,
    objects: &mut HashSet<(u64, u64)>,
    mounts: &Mounts,
    depth: usize,
) -> io::Result<()> {
    for child in std::fs::read_dir(&entry.path)? {
        scan(
            &crate::fs::inspect(&child?.path()),
            objects,
            mounts,
            depth + 1,
        )?;
    }
    Ok(())
}
