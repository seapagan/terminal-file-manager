use crate::fs::Entry;
use std::{io, path::PathBuf};
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxTrash;

/// Native recovery evidence, never a fabricated TFM recovery directory.
#[derive(Clone, Debug)]
pub struct Receipt {
    pub path: PathBuf,
    pub info: PathBuf,
}

pub(super) fn put(entry: &Entry) -> io::Result<Receipt> {
    #[cfg(target_os = "linux")]
    {
        let data = match std::env::var_os("XDG_DATA_HOME") {
            Some(path) if !path.is_empty() => PathBuf::from(path),
            _ => PathBuf::from(std::env::var_os("HOME").ok_or_else(|| {
                io::Error::other("HOME/XDG_DATA_HOME unavailable; Trash unavailable")
            })?)
            .join(".local/share"),
        };
        LinuxTrash::new(data).put(entry)
    }
    #[cfg(any(target_os = "macos", windows))]
    {
        let item = junkyard::discard(&entry.path).map_err(io::Error::other)?;
        Ok(Receipt {
            path: item.id().into(),
            info: PathBuf::new(),
        })
    }
}
