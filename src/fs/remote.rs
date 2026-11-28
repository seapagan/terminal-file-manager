//! Adapt SFTP metadata to the common listing without assigning a native path.
use super::{Entry, EntryKind};
use russh_sftp::{
    client::fs::DirEntry,
    protocol::{FileAttributes, SftpString},
};
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RemoteEntry {
    pub path: SftpString,
    pub name: SftpString,
    pub order: usize,
    uid: Option<u32>,
    gid: Option<u32>,
}

impl RemoteEntry {
    pub(super) fn owner_display(&self) -> String {
        fn id(value: Option<u32>) -> String {
            value.map_or_else(|| "unavailable".into(), |value| value.to_string())
        }
        if self.uid.is_none() && self.gid.is_none() {
            "unavailable".into()
        } else {
            format!("{}:{}", id(self.uid), id(self.gid))
        }
    }
}

impl Entry {
    pub(crate) fn from_sftp(entry: DirEntry, order: usize) -> Self {
        Self::from_sftp_metadata(entry.path(), entry.file_name(), entry.metadata(), order)
    }

    pub(crate) fn from_sftp_metadata(
        path: SftpString,
        name: SftpString,
        attrs: FileAttributes,
        order: usize,
    ) -> Self {
        let kind = kind(attrs.permissions);
        let time = |seconds: u32| {
            SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(u64::from(seconds)))
        };
        Self {
            path: None,
            name: String::from_utf8_lossy(name.as_bytes()).as_ref().into(),
            hidden: name.as_bytes().starts_with(b"."),
            remote: Some(RemoteEntry {
                path,
                name,
                order,
                uid: attrs.uid,
                gid: attrs.gid,
            }),
            kind,
            identity: None,
            size: attrs.size,
            modified: attrs.mtime.and_then(time),
            created: None,
            accessed: attrs.atime.and_then(time),
            #[cfg(unix)]
            attributes: String::new(),
            windows_attributes: None,
            mode: attrs.permissions,
            owner: None,
            #[cfg(windows)]
            windows_owner: crate::owner::windows::Ownership::Pending,
            executable: kind == EntryKind::File
                && attrs.permissions.is_some_and(|m| m & 0o111 != 0),
            enterable: kind == EntryKind::Directory,
            link_target: None,
            ignored: None,
            error: None,
        }
    }
}

fn kind(permissions: Option<u32>) -> EntryKind {
    match permissions.map(|mode| mode & 0o170000) {
        Some(0o040000) => EntryKind::Directory,
        Some(0o100000) => EntryKind::File,
        Some(0o120000) => EntryKind::Link,
        Some(0o140000) => EntryKind::Socket,
        Some(0o010000) => EntryKind::Fifo,
        Some(0o020000 | 0o060000) => EntryKind::Device,
        _ => EntryKind::Unknown,
    }
}
