use super::{
    Operation,
    safety::{self, Result},
};
use crate::fs::{Entry, EntryKind};
use std::{collections::BTreeSet, fs, io, path::PathBuf};

pub(super) struct Mounts {
    enabled: bool,
    roots: BTreeSet<PathBuf>,
}

impl Mounts {
    pub fn capture(operation: Operation) -> io::Result<Self> {
        let enabled = matches!(
            operation,
            Operation::Move | Operation::Delete | Operation::Trash
        );
        Ok(Self {
            enabled,
            roots: mount_roots(enabled)?,
        })
    }

    pub fn reject(&self, source: &Entry) -> Result<()> {
        if !self.enabled || !matches!(source.kind, EntryKind::File | EntryKind::Directory) {
            return Ok(());
        }
        if self.roots.contains(&fs::canonicalize(&source.path)?) || different_volume(source)? {
            return Err(safety::changed(
                "Mounted object refused; select its contents explicitly",
            ));
        }
        Ok(())
    }
}

#[cfg(unix)]
fn different_volume(source: &Entry) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let parent = source
        .path
        .parent()
        .ok_or_else(|| io::Error::other("Root mutation refused"))?;
    let device = fs::metadata(parent)?.dev();
    let object = source
        .identity
        .as_ref()
        .and_then(|identity| identity.object)
        .ok_or_else(|| io::Error::other("Source device identity unavailable; mutation refused"))?;
    Ok(object.0 != device)
}

#[cfg(windows)]
fn different_volume(_source: &Entry) -> io::Result<bool> {
    Ok(false)
}

#[cfg(target_os = "linux")]
fn mount_roots(enabled: bool) -> io::Result<BTreeSet<PathBuf>> {
    if !enabled {
        return Ok(BTreeSet::new());
    }
    parse_mountinfo(&fs::read("/proc/self/mountinfo")?)
}

#[cfg(not(target_os = "linux"))]
fn mount_roots(_enabled: bool) -> io::Result<BTreeSet<PathBuf>> {
    Ok(BTreeSet::new())
}

#[cfg(target_os = "linux")]
fn parse_mountinfo(bytes: &[u8]) -> io::Result<BTreeSet<PathBuf>> {
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            let field = line
                .split(|byte| *byte == b' ')
                .nth(4)
                .ok_or_else(|| io::Error::other("Malformed mountinfo record"))?;
            decode_path(field)
        })
        .collect()
}

#[cfg(target_os = "linux")]
pub(super) fn decode_path(field: &[u8]) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let mut bytes = Vec::with_capacity(field.len());
    let mut position = 0;
    while position < field.len() {
        if field[position] == b'\\' {
            let digits = field
                .get(position + 1..position + 4)
                .ok_or_else(|| io::Error::other("Truncated mountinfo escape"))?;
            let value = match digits {
                b"040" => b' ',
                b"011" => b'\t',
                b"012" => b'\n',
                b"134" => b'\\',
                _ => return Err(io::Error::other("Invalid mountinfo escape")),
            };
            bytes.push(value);
            position += 4;
        } else {
            bytes.push(field[position]);
            position += 1;
        }
    }
    let path = PathBuf::from(std::ffi::OsString::from_vec(bytes));
    if !path.is_absolute() {
        return Err(io::Error::other("Non-absolute mountinfo path"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn missing_device_identity_is_an_explicit_failure_not_a_mount_mismatch() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::write(&source, "preserved").unwrap();
        let mut entry = crate::fs::inspect(&source);
        entry.identity.as_mut().unwrap().object = None;
        let mounts = Mounts {
            enabled: true,
            roots: BTreeSet::new(),
        };
        let error = mounts.reject(&entry).unwrap_err();
        assert_eq!(error.outcome, super::super::Outcome::Failed);
        assert_eq!(
            error.detail,
            "Source device identity unavailable; mutation refused"
        );
        assert_eq!(fs::read(source).unwrap(), b"preserved");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mountinfo_decodes_native_escaped_names_and_rejects_malformed_records() {
        let roots =
            parse_mountinfo(b"42 1 8:1 / /with\\040space\\011tab\\134slash rw - ext4 /dev/a rw\n")
                .unwrap();
        assert!(roots.contains(std::path::Path::new("/with space\ttab\\slash")));
        assert!(parse_mountinfo(b"broken mount record\n").is_err());
        assert!(parse_mountinfo(b"42 1 8:1 / /bad\\xyz rw - ext4 /dev/a rw\n").is_err());
    }

    #[test]
    fn a_known_mount_root_is_refused_without_touching_its_owned_contents() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("sentinel"), "preserved").unwrap();
        let physical = std::fs::canonicalize(root.path()).unwrap();
        let mounts = Mounts {
            enabled: true,
            roots: std::collections::BTreeSet::from([physical]),
        };
        assert!(mounts.reject(&crate::fs::inspect(root.path())).is_err());
        assert_eq!(
            std::fs::read(root.path().join("sentinel")).unwrap(),
            b"preserved"
        );
    }
}
