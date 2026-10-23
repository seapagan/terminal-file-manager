use crate::operations::mounts::decode_path;
use std::{
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

/// Resolve the owning mount without following the selected final object.
pub(super) fn topdir(source: &Path) -> io::Result<PathBuf> {
    let bytes = fs::read("/proc/self/mountinfo")?;
    resolve(source, &bytes)
}

fn resolve(source: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let physical = crate::operations::locks::namespace(source)?;
    let source = physical.as_path();
    let parent = source
        .parent()
        .ok_or_else(|| io::Error::other("Root Trash refused"))?;
    let records = records(bytes)?;
    let top = selected_mount(&records, parent)?;
    let device = fs::symlink_metadata(source)?.dev();
    if fs::metadata(top)?.dev() != device {
        return Err(io::Error::other(
            "Mount/device identity differs; Trash refused",
        ));
    }
    reject_aliases(&records, source, top, device)?;
    Ok(top.to_path_buf())
}

fn selected_mount<'a>(records: &'a [(bool, PathBuf)], parent: &Path) -> io::Result<&'a Path> {
    let (_, top) = records
        .iter()
        .filter(|(_, top)| parent.starts_with(top))
        .max_by_key(|(_, top)| top.components().count())
        .ok_or_else(|| io::Error::other("Owning mount unavailable"))?;
    let matches: Vec<_> = records.iter().filter(|(_, path)| path == top).collect();
    if matches.len() != 1 || !matches[0].0 {
        return Err(io::Error::other(
            "Ambiguous or subtree bind mount; Trash refused",
        ));
    }
    Ok(top)
}

fn reject_aliases(
    records: &[(bool, PathBuf)],
    source: &Path,
    top: &Path,
    device: u64,
) -> io::Result<()> {
    for (_, alias) in records
        .iter()
        .filter(|(_, path)| path != top && source.starts_with(path))
    {
        if fs::metadata(alias).is_ok_and(|metadata| metadata.dev() == device) {
            return Err(io::Error::other("Aliased filesystem mounts; Trash refused"));
        }
    }
    Ok(())
}

fn records(bytes: &[u8]) -> io::Result<Vec<(bool, PathBuf)>> {
    let mut records = Vec::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let fields: Vec<_> = line.split(|byte| *byte == b' ').collect();
        if fields.len() < 10 || !fields.contains(&b"-".as_slice()) {
            return Err(io::Error::other("Malformed Trash mount information"));
        }
        records.push((fields[3] == b"/", decode_path(fields[4])?));
    }
    Ok(records)
}

pub(super) fn existing_device(path: &Path) -> io::Result<u64> {
    let mut ancestor = path;
    loop {
        match fs::metadata(ancestor) {
            Ok(metadata) => return Ok(metadata.dev()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                ancestor = ancestor.parent().ok_or(error)?;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrelated_same_device_mounts_do_not_disable_volume_trash() {
        let root = tempfile::tempdir().unwrap();
        let top = root.path().join("volume");
        let unrelated = root.path().join("volume-alias");
        fs::create_dir(&top).unwrap();
        fs::create_dir(&unrelated).unwrap();
        let source = top.join("source");
        fs::write(&source, "preserved").unwrap();
        let bytes = format!(
            "42 1 0:1 / {} rw - tmpfs none rw\n43 1 0:1 /subtree {} rw - tmpfs none rw\n",
            top.display(),
            unrelated.display()
        );
        assert_eq!(resolve(&source, bytes.as_bytes()).unwrap(), top);
        let alias = root.path().join("physical-parent-alias");
        std::os::unix::fs::symlink(&top, &alias).unwrap();
        assert_eq!(
            resolve(&alias.join("source"), bytes.as_bytes()).unwrap(),
            top
        );
        assert!(alias.is_symlink());
        assert_eq!(fs::read(source).unwrap(), b"preserved");
    }

    #[test]
    fn source_ancestor_aliases_and_subtree_mounts_remain_refused() {
        let root = tempfile::tempdir().unwrap();
        let top = root.path().join("volume");
        fs::create_dir(&top).unwrap();
        let source = top.join("source");
        fs::write(&source, "preserved").unwrap();
        let bytes = format!(
            "42 1 0:1 / {} rw - tmpfs none rw\n43 1 0:1 / {} rw - tmpfs none rw\n",
            top.display(),
            root.path().display()
        );
        assert!(
            resolve(&source, bytes.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("Aliased filesystem")
        );
        let subtree = format!("42 1 0:1 /subtree {} rw - tmpfs none rw\n", top.display());
        assert!(resolve(&source, subtree.as_bytes()).is_err());
        assert_eq!(fs::read(source).unwrap(), b"preserved");
    }

    #[test]
    fn mount_resolution_refuses_subtree_bind_and_ambiguous_stacked_mounts() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("file");
        fs::write(&source, "preserved").unwrap();
        let record = format!(
            "42 1 0:1 /subtree {} rw - tmpfs none rw\n",
            root.path().display()
        );
        assert!(resolve(&source, record.as_bytes()).is_err());
        let record = record.replace("/subtree", "/").repeat(2);
        assert!(resolve(&source, record.as_bytes()).is_err());
        assert_eq!(fs::read(source).unwrap(), b"preserved");
    }
}
