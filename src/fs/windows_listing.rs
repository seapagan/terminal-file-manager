//! Transient local-NTFS enumeration; operation-time inspection is independent.
use super::{
    Entry, EntryKind, Identity,
    directory_records::{self, Record},
    windows_native as native,
};
use std::{
    ffi::OsString,
    fs, io,
    os::windows::{ffi::OsStringExt, fs::MetadataExt},
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, SystemTime},
};

pub(super) struct Listing {
    pub entries: Vec<Entry>,
    pub errors: Vec<String>,
}

pub(super) fn list(
    path: &Path,
    initial: Option<&Entry>,
    cancelled: &AtomicBool,
) -> io::Result<Option<Listing>> {
    super::check_cancelled(cancelled)?;
    let Some(initial) = initial.filter(|entry| entry.created.is_some()) else {
        return Ok(None);
    };
    if has_reparse_ancestor(path)? {
        return Ok(None);
    }
    let file = native::open_directory(path)?;
    let Some(volume) = native::query_directory_volume(&file)? else {
        return Ok(None);
    };
    verify_binding(&file, volume, initial)?;
    let rows = enumerate(&file, cancelled)?;
    drop(file);
    let Some((rows, errors)) = rows else {
        return Ok(None);
    };
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        super::check_cancelled(cancelled)?;
        entries.push(entry(path, volume, row));
    }
    Ok(Some(Listing { entries, errors }))
}

fn has_reparse_ancestor(path: &Path) -> io::Result<bool> {
    for ancestor in std::path::absolute(path)?.ancestors() {
        if fs::symlink_metadata(ancestor)?.file_attributes() & 0x400 != 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn verify_binding(file: &fs::File, volume: u64, initial: &Entry) -> io::Result<()> {
    let info = winapi_util::file::information(file)?;
    let object = (info.volume_serial_number(), info.file_index());
    let created = file.metadata()?.created().ok();
    let same = initial
        .identity
        .as_ref()
        .is_some_and(|id| id.object == Some(object) && id.created == created);
    if volume != object.0 || object.1 == 0 || !same {
        return Err(io::Error::other(
            "Directory identity changed during buffered enumeration",
        ));
    }
    Ok(())
}

type Records = (Vec<Record>, Vec<String>);

fn enumerate(file: &fs::File, cancelled: &AtomicBool) -> io::Result<Option<Records>> {
    let mut buffer = Box::new(native::DirectoryBuffer([0; 65536]));
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    loop {
        super::check_cancelled(cancelled)?;
        match native::read_directory_batch(file, &mut buffer) {
            Ok(false) => break,
            Ok(true) => rows.extend(directory_records::parse(&buffer.0)?),
            Err(error) if native::unsupported(&error) => return Ok(None),
            Err(error) if rows.is_empty() => return Err(error),
            Err(error) => {
                errors.push(error.to_string());
                break;
            }
        }
    }
    Ok(Some((rows, errors)))
}

fn entry(parent: &Path, volume: u64, row: Record) -> Entry {
    let name = OsString::from_wide(&row.name);
    let path = parent.join(&name);
    let created = timestamp(row.created);
    let modified = timestamp(row.modified);
    let accessed = timestamp(row.accessed);
    if row.attributes & 0x400 != 0
        || row.id == 0
        || created.is_none()
        || row.created <= 0
        || row.modified < 0
        || row.accessed < 0
        || row.length > i64::MAX as u64
    {
        return super::inspect(&path);
    }
    let directory = row.attributes & 0x10 != 0;
    Entry {
        path,
        name,
        kind: if directory {
            EntryKind::Directory
        } else {
            EntryKind::File
        },
        identity: Some(Identity {
            object: Some((volume, row.id)),
            created,
            modified,
            length: row.length,
        }),
        size: Some(row.length),
        modified,
        created,
        accessed,
        attributes: format!("0x{:08x}", row.attributes),
        mode: None,
        owner: None,
        hidden: row.attributes & 2 != 0,
        executable: false,
        enterable: directory,
        link_target: None,
        ignored: None,
        error: None,
    }
}

fn timestamp(ticks: i64) -> Option<SystemTime> {
    const EPOCH: i64 = 116_444_736_000_000_000;
    let delta = i128::from(ticks) - i128::from(EPOCH);
    let magnitude = u64::try_from(delta.abs()).ok()?;
    let duration = Duration::new(
        magnitude / 10_000_000,
        ((magnitude % 10_000_000) * 100) as u32,
    );
    if delta < 0 {
        SystemTime::UNIX_EPOCH.checked_sub(duration)
    } else {
        SystemTime::UNIX_EPOCH.checked_add(duration)
    }
}

#[cfg(test)]
mod tests;
