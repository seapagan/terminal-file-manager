//! Native metadata boundaries, informed by lsplus's no-follow classification.

use super::EntryKind;
use std::{ffi::OsStr, fs::Metadata};

#[cfg(unix)]
pub(super) fn kind(metadata: &Metadata) -> EntryKind {
    use std::os::unix::fs::FileTypeExt;
    let kind = metadata.file_type();
    if kind.is_symlink() {
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
    }
}

#[cfg(windows)]
pub(super) fn kind(metadata: &Metadata) -> EntryKind {
    use std::os::windows::fs::MetadataExt;
    if metadata.is_symlink() {
        EntryKind::Link
    } else if metadata.file_attributes() & 0x400 != 0 {
        EntryKind::ReparsePoint
    } else if metadata.is_dir() {
        EntryKind::Directory
    } else if metadata.is_file() {
        EntryKind::File
    } else {
        EntryKind::Unknown
    }
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
    format!("{:04o}", metadata.permissions().mode() & 0o7777)
}

#[cfg(windows)]
pub(super) fn attributes(metadata: &Metadata) -> String {
    use std::os::windows::fs::MetadataExt;
    format!("0x{:08x}", metadata.file_attributes())
}

#[cfg(unix)]
pub(super) fn object(metadata: &Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
pub(super) fn object(_metadata: &Metadata) -> Option<(u64, u64)> {
    None
}

#[cfg(unix)]
pub(super) fn owner(metadata: &Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    Some(format!("{}:{} (uid:gid)", metadata.uid(), metadata.gid()))
}

#[cfg(windows)]
pub(super) fn owner(_metadata: &Metadata) -> Option<String> {
    None
}
