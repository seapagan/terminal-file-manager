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
    Mac { path: Option<PathBuf> },
    Windows { parsing_name: Option<Vec<u16>> },
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
    #[cfg(any(target_os = "macos", windows))]
    {
        crate::fs::check_cancelled(cancel)?;
        let item = junkyard::discard(&entry.path).map_err(|error| {
            let outcome = if super::safety::revalidate(entry, true).is_ok() {
                super::Outcome::Failed
            } else {
                super::Outcome::OutcomeUnknown
            };
            super::safety::Failure {
                outcome,
                detail: format!("Native Trash error; no automatic retry: {error}"),
            }
        })?;
        #[cfg(target_os = "macos")]
        return Ok(Receipt::Mac {
            path: Some(item.id().into()),
        });
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            Ok(Receipt::Windows {
                parsing_name: Some(std::ffi::OsStr::new(item.id()).encode_wide().collect()),
            })
        }
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
