//! Read-only local filesystem snapshots. Native paths never come from display text.

#[cfg(any(windows, test))]
mod directory_records;
mod platform;
mod rules;
mod totals;
#[cfg(windows)]
mod windows_listing;
#[cfg(windows)]
mod windows_native;

pub use rules::IgnoreContext;
use std::ffi::OsString;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;
pub use totals::{Totals, totals};

/// No-follow classification; links may be entered but never recursively scanned.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EntryKind {
    Directory,
    File,
    Link,
    Junction,
    ReparsePoint,
    Socket,
    Fifo,
    Device,
    Unknown,
}

/// Available native object identity and freshness evidence.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Identity {
    pub object: Option<(u64, u64)>,
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub length: u64,
}

/// Cached entry metadata. Missing fields are unavailable, never invented.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: OsString,
    pub kind: EntryKind,
    pub identity: Option<Identity>,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub attributes: String,
    pub mode: Option<u32>,
    pub owner: Option<crate::owner::Ownership>,
    pub hidden: bool,
    pub executable: bool,
    pub enterable: bool,
    pub link_target: Option<PathBuf>,
    pub ignored: Option<bool>,
    pub error: Option<String>,
}

impl Entry {
    /// Format cached account data without resolving accounts on the render thread.
    pub fn owner_display(&self, mode: crate::owner::OwnerDisplay) -> String {
        self.owner
            .as_ref()
            .map_or_else(|| "unavailable".into(), |owner| owner.display(mode))
    }

    /// Recursive inspection never follows links or reparse points.
    pub fn may_recurse(&self) -> bool {
        self.kind == EntryKind::Directory
    }
}

/// One atomic directory result, with explicit partial/ignore errors.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// Physical directory identity for later mutation approvals; absent if unresolved/changed.
    pub directory: Option<Box<Entry>>,
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub errors: Vec<String>,
}

/// Read a directory without opening file contents or mutating user data.
///
/// # Errors
/// Returns the selected directory's error, or Interrupted on cancellation.
pub fn list(path: &Path, context: &IgnoreContext, cancelled: &AtomicBool) -> io::Result<Snapshot> {
    #[cfg(windows)]
    return list_backend(path, context, cancelled, true);
    #[cfg(not(windows))]
    list_backend(path, context, cancelled)
}

fn list_backend(
    path: &Path,
    context: &IgnoreContext,
    cancelled: &AtomicBool,
    #[cfg(windows)] buffered: bool,
) -> io::Result<Snapshot> {
    let mut owners = crate::owner::Cache::default();
    let initial_directory = directory_identity(path, &mut owners);
    #[cfg(windows)]
    if buffered
        && let Some(listing) = windows_listing::list(path, initial_directory.as_ref(), cancelled)?
    {
        return finish_buffered(path, context, cancelled, initial_directory, listing);
    }
    list_ordinary(path, context, cancelled, initial_directory, &mut owners)
}

fn list_ordinary(
    path: &Path,
    context: &IgnoreContext,
    cancelled: &AtomicBool,
    initial_directory: Option<Entry>,
    owners: &mut crate::owner::Cache,
) -> io::Result<Snapshot> {
    let directory = fs::read_dir(path)?;
    let (rules, mut errors) = rules::load(path, context);
    let mut entries = Vec::new();
    for item in directory {
        check_cancelled(cancelled)?;
        match item {
            Ok(item) => entries.push(inspect_with_cache(&item.path(), owners)),
            Err(error) => errors.push(error.to_string()),
        }
    }
    for entry in &mut entries {
        check_cancelled(cancelled)?;
        entry.ignored = rules
            .as_ref()
            .map(|rules| rules.ignored(&entry.path, entry.may_recurse()));
    }
    check_cancelled(cancelled)?;
    let directory = verified_directory(initial_directory, directory_identity(path, owners));
    Ok(Snapshot {
        directory,
        path: path.to_path_buf(),
        entries,
        errors,
    })
}

#[cfg(windows)]
fn finish_buffered(
    path: &Path,
    context: &IgnoreContext,
    cancelled: &AtomicBool,
    initial: Option<Entry>,
    mut listing: windows_listing::Listing,
) -> io::Result<Snapshot> {
    let (rules, mut errors) = rules::load(path, context);
    errors.append(&mut listing.errors);
    for entry in &mut listing.entries {
        check_cancelled(cancelled)?;
        entry.ignored = rules
            .as_ref()
            .map(|rules| rules.ignored(&entry.path, entry.may_recurse()));
    }
    check_cancelled(cancelled)?;
    let directory = verified_directory(
        initial,
        directory_identity(path, &mut crate::owner::Cache::default()),
    );
    Ok(Snapshot {
        directory,
        path: path.to_path_buf(),
        entries: listing.entries,
        errors,
    })
}

fn verified_directory(before: Option<Entry>, after: Option<Entry>) -> Option<Box<Entry>> {
    before.zip(after).and_then(|(before, after)| {
        (before.path == after.path
            && before
                .identity
                .as_ref()
                .zip(after.identity.as_ref())
                .is_some_and(|(old, new)| {
                    old.object.is_some() && old.object == new.object && old.created == new.created
                }))
        .then_some(Box::new(before))
    })
}

fn directory_identity(path: &Path, owners: &mut crate::owner::Cache) -> Option<Entry> {
    let canonical = fs::canonicalize(path).ok()?;
    let entry = inspect_no_follow(&canonical, Some(owners));
    (entry.kind == EntryKind::Directory
        && entry.identity.as_ref().and_then(|id| id.object).is_some())
    .then_some(entry)
}

pub(crate) fn check_cancelled(cancelled: &AtomicBool) -> io::Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "inspection cancelled",
        ))
    } else {
        Ok(())
    }
}

/// Inspect an entry itself, retaining broken links and metadata errors.
pub fn inspect(path: &Path) -> Entry {
    inspect_with_cache(path, &mut crate::owner::Cache::default())
}

fn inspect_with_cache(path: &Path, owners: &mut crate::owner::Cache) -> Entry {
    let mut entry = inspect_no_follow(path, Some(owners));
    if matches!(entry.kind, EntryKind::Link | EntryKind::Junction) {
        entry.link_target = fs::read_link(&entry.path).ok();
        match fs::metadata(&entry.path) {
            Ok(target) => entry.enterable = target.is_dir(),
            Err(error) => {
                entry.enterable = false;
                entry.error = Some(error.to_string());
            }
        }
    }
    entry
}

// Totals share no-follow classification and diagnostics without resolving accounts.
fn inspect_no_follow(path: &Path, owners: Option<&mut crate::owner::Cache>) -> Entry {
    let mut entry = Entry {
        path: path.to_path_buf(),
        name: path.file_name().unwrap_or(path.as_os_str()).to_owned(),
        kind: EntryKind::Unknown,
        identity: None,
        size: None,
        modified: None,
        created: None,
        accessed: None,
        attributes: String::new(),
        mode: None,
        owner: None,
        hidden: false,
        executable: false,
        enterable: false,
        link_target: None,
        ignored: None,
        error: None,
    };
    match fs::symlink_metadata(path) {
        Ok(metadata) => fill(&mut entry, &metadata, owners),
        Err(error) => entry.error = Some(error.to_string()),
    }
    entry
}

fn fill(entry: &mut Entry, metadata: &Metadata, owners: Option<&mut crate::owner::Cache>) {
    entry.kind = platform::kind(&entry.path, metadata).unwrap_or_else(|error| {
        entry.error = Some(error.to_string());
        EntryKind::ReparsePoint
    });
    entry.hidden = platform::hidden(&entry.name, metadata);
    entry.executable = platform::executable(metadata);
    entry.attributes = platform::attributes(metadata);
    entry.mode = platform::mode(metadata);
    entry.owner = owners.and_then(|owners| platform::owner(metadata, owners));
    entry.size = Some(metadata.len());
    entry.modified = metadata.modified().ok();
    entry.created = metadata.created().ok();
    entry.accessed = metadata.accessed().ok();
    entry.identity = Some(Identity {
        object: platform::object(&entry.path, metadata).unwrap_or_else(|error| {
            entry.error = Some(error.to_string());
            None
        }),
        created: entry.created,
        modified: entry.modified,
        length: metadata.len(),
    });
    entry.enterable = metadata.is_dir();
    if entry.kind == EntryKind::ReparsePoint {
        entry.enterable = false;
        entry
            .error
            .get_or_insert_with(|| "Unsupported reparse point; navigation disabled".into());
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use std::time::Duration;

    fn before_after() -> (Entry, Entry) {
        let root = tempfile::tempdir().unwrap();
        let mut entry =
            directory_identity(root.path(), &mut crate::owner::Cache::default()).unwrap();
        entry.created = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1));
        entry.identity.as_mut().unwrap().created = entry.created;
        (entry.clone(), entry)
    }

    #[test]
    fn recycled_directory_object_id_with_new_creation_stamp_is_not_verified() {
        let (before, mut after) = before_after();
        after.created = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(2));
        after.identity.as_mut().unwrap().created = after.created;
        assert!(verified_directory(Some(before), Some(after)).is_none());
    }

    #[test]
    fn directory_activity_during_enumeration_keeps_unchanged_object_evidence() {
        let (before, mut after) = before_after();
        let identity = after.identity.as_mut().unwrap();
        identity.length += 1;
        identity.modified = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(3));
        after.size = Some(identity.length);
        after.modified = identity.modified;
        assert!(verified_directory(Some(before), Some(after)).is_some());
    }
}
