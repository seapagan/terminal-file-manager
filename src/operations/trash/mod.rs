#[cfg(any(windows, test))]
mod acknowledgement;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
use crate::fs::Entry;
#[cfg(target_os = "linux")]
use std::io;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod volume;
#[cfg(target_os = "linux")]
pub use linux::LinuxTrash;

/// Native recovery evidence, never a fabricated TFM recovery directory.
#[derive(Clone, Debug)]
pub struct LinuxReceipt {
    pub path: PathBuf,
    pub info: PathBuf,
}

/// Recovery references retain each OS's native namespace semantics.
#[derive(Clone, Debug)]
pub enum Receipt {
    Linux(LinuxReceipt),
    Mac {
        path: Option<PathBuf>,
    },
    Windows {
        parsing_name: Option<Vec<u16>>,
        metadata: ShellMetadata,
    },
}

/// Optional owned properties read from the live recycled Shell item.
#[derive(Clone, Debug, Default)]
pub struct ShellMetadata {
    pub display_name: Option<Vec<u16>>,
    pub deleted_from: Option<Vec<u16>>,
    /// Native Windows FILETIME ticks; absent when the Shell property is unavailable.
    pub deletion_time: Option<u64>,
}

impl Receipt {
    /// Only filesystem-backed native references can be inspected as paths.
    pub fn path(&self) -> Option<&std::path::Path> {
        match self {
            Self::Linux(item) => Some(&item.path),
            Self::Mac { path } => path.as_deref(),
            Self::Windows { .. } => None,
        }
    }
    pub(crate) fn description(&self) -> String {
        match self.path() {
            Some(path) => format!("Recycled; recovery location {}", path.display()),
            None => "Recycled; native acknowledgement, limited recovery metadata".into(),
        }
    }
}

pub(super) fn put(
    entry: &Entry,
    cancel: &std::sync::atomic::AtomicBool,
) -> super::safety::Result<Receipt> {
    #[cfg(target_os = "linux")]
    {
        LinuxTrash::new(data_root()?)
            .put_checked(entry, cancel)
            .map(Receipt::Linux)
    }
    #[cfg(target_os = "macos")]
    {
        macos::put(entry, cancel)
    }
    #[cfg(windows)]
    {
        windows::put(entry, cancel)
    }
}

#[cfg(target_os = "linux")]
pub(super) fn data_root() -> io::Result<PathBuf> {
    let root = match std::env::var_os("XDG_DATA_HOME") {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from(std::env::var_os("HOME").ok_or_else(|| {
            io::Error::other("HOME/XDG_DATA_HOME unavailable; Trash unavailable")
        })?)
        .join(".local/share"),
    };
    if !root.is_absolute() {
        return Err(io::Error::other(
            "Relative XDG_DATA_HOME; Trash unavailable",
        ));
    }
    Ok(root)
}

#[cfg(target_os = "linux")]
pub(super) fn claim_roots(entry: &Entry) -> io::Result<Vec<PathBuf>> {
    let mut roots = vec![data_root()?.join("Trash")];
    let source = super::locks::namespace(&entry.path)?;
    use std::os::unix::fs::MetadataExt;
    if volume::existing_device(&data_root()?)? == std::fs::symlink_metadata(&source)?.dev() {
        return Ok(roots);
    }
    let top = volume::topdir(&source)?;
    let uid = nix::unistd::geteuid();
    roots.push(top.join(".Trash").join(uid.to_string()));
    roots.push(top.join(format!(".Trash-{uid}")));
    Ok(roots)
}
