//! Native metadata boundaries, informed by lsplus's no-follow classification.

use super::EntryKind;
use std::{ffi::OsStr, fs::Metadata, io, path::Path};

#[cfg(unix)]
pub(super) fn kind(_path: &Path, metadata: &Metadata) -> io::Result<EntryKind> {
    use std::os::unix::fs::FileTypeExt;
    let kind = metadata.file_type();
    Ok(if kind.is_symlink() {
        EntryKind::Link
    } else if kind.is_dir() {
        EntryKind::Directory
    } else if kind.is_file() {
        EntryKind::File
    } else if kind.is_socket() {
        EntryKind::Socket
    } else if kind.is_fifo() {
        EntryKind::Fifo
    } else if kind.is_block_device() || kind.is_char_device() {
        EntryKind::Device
    } else {
        EntryKind::Unknown
    })
}

#[cfg(windows)]
pub(super) fn kind(path: &Path, metadata: &Metadata) -> io::Result<EntryKind> {
    use std::os::windows::fs::MetadataExt;
    if metadata.file_attributes() & 0x400 != 0 {
        return Ok(match super::windows_native::reparse_tag(path)? {
            0xA000_000C => EntryKind::Link,
            0xA000_0003 => EntryKind::Junction,
            _ => EntryKind::ReparsePoint,
        });
    }
    Ok(if metadata.is_dir() {
        EntryKind::Directory
    } else if metadata.is_file() {
        EntryKind::File
    } else {
        EntryKind::Unknown
    })
}

#[cfg(unix)]
pub(super) fn hidden(name: &OsStr, _metadata: &Metadata) -> bool {
    use std::os::unix::ffi::OsStrExt;
    name.as_bytes().starts_with(b".")
}

#[cfg(windows)]
pub(super) fn hidden(_name: &OsStr, metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 2 != 0
}

#[cfg(unix)]
pub(super) fn executable(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

#[cfg(windows)]
pub(super) fn executable(_metadata: &Metadata) -> bool {
    false
}

#[cfg(unix)]
pub(super) fn attributes(metadata: &Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    crate::permissions::mode_to_octal(metadata.permissions().mode())
}

#[cfg(unix)]
pub(super) fn mode(metadata: &Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(metadata.permissions().mode())
}

#[cfg(windows)]
pub(super) fn mode(_metadata: &Metadata) -> Option<u32> {
    None
}

#[cfg(windows)]
pub(super) fn attributes(metadata: &Metadata) -> String {
    use std::os::windows::fs::MetadataExt;
    format!("0x{:08x}", metadata.file_attributes())
}

#[cfg(unix)]
pub(super) fn object(_path: &Path, metadata: &Metadata) -> io::Result<Option<(u64, u64)>> {
    use std::os::unix::fs::MetadataExt;
    Ok(Some((metadata.dev(), metadata.ino())))
}

#[cfg(windows)]
pub(super) fn object(path: &Path, _metadata: &Metadata) -> io::Result<Option<(u64, u64)>> {
    super::windows_native::object_id(path).map(Some)
}

#[cfg(unix)]
pub(super) fn owner(metadata: &Metadata) -> Option<crate::owner::Ownership> {
    use std::os::unix::fs::MetadataExt;
    Some(crate::owner::lookup(metadata.uid(), metadata.gid()))
}

#[cfg(windows)]
pub(super) fn owner(_metadata: &Metadata) -> Option<crate::owner::Ownership> {
    None
}
