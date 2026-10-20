use super::Receipt;
use crate::{
    fs::Entry,
    operations::{
        safety::{self, Parents},
        staging,
    },
};
use std::{
    fs::{self, DirBuilder},
    io::{self, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

/// Freedesktop home Trash with injectable data root and no copy/delete fallback.
pub struct LinuxTrash {
    data: PathBuf,
}

impl LinuxTrash {
    /// The XDG data root must be absolute; tests supply an owned temporary root.
    pub fn new(data: PathBuf) -> Self {
        Self { data }
    }

    /// Recycle an exact no-follow object through same-filesystem native rename.
    ///
    /// # Errors
    /// Rejects unsafe ownership/permissions, cross-device paths, changed identity or uncertain rename.
    pub fn put(&self, entry: &Entry) -> io::Result<Receipt> {
        self.put_checked(entry)
            .map_err(|failure| io::Error::other(failure.detail))
    }

    fn put_checked(&self, entry: &Entry) -> safety::Result<Receipt> {
        let source_guard = Parents::capture(&entry.path)?;
        safety::revalidate(entry, true)?;
        validate_scope(entry)?;
        let (files, info) = self.prepare(entry)?;
        let (destination, info_path) = reserve(entry, &files, &info)?;
        let info_entry = crate::fs::inspect(&info_path);
        let target_guard = Parents::capture(&destination)?;
        let info_guard = Parents::capture(&info_path)?;
        let result = commit(
            entry,
            &destination,
            &info_entry,
            [&source_guard, &target_guard, &info_guard],
        );
        if let Err(error) = result {
            cleanup_info(entry, &destination, &info_entry);
            return Err(error);
        }
        Ok(Receipt {
            path: destination,
            info: info_path,
        })
    }

    fn prepare(&self, source: &Entry) -> safety::Result<(PathBuf, PathBuf)> {
        validate_data(&self.data, source)?;
        fs::create_dir_all(&self.data)?;
        let trash = self.data.join("Trash");
        private_directory(&trash)?;
        let files = trash.join("files");
        let info = trash.join("info");
        private_directory(&files)?;
        private_directory(&info)?;
        let source_device = fs::symlink_metadata(&source.path)?.dev();
        if source_device != fs::symlink_metadata(&files)?.dev() {
            return Err(io::Error::other(
                "Cross-filesystem Trash unsupported; no permanent-delete fallback",
            )
            .into());
        }
        safety::reject_overlap(source, &files.join("reservation"))?;
        Ok((files, info))
    }
}

fn validate_scope(entry: &Entry) -> safety::Result<()> {
    let plan = crate::operations::Plan::paths(
        crate::operations::Operation::Trash,
        vec![entry.path.clone()],
        None,
    )?;
    crate::operations::Preview::capture(&plan, &false.into())?;
    Ok(())
}

fn validate_data(data: &Path, source: &Entry) -> io::Result<()> {
    if !data.is_absolute() {
        return Err(io::Error::other(
            "Relative XDG_DATA_HOME; Trash unavailable",
        ));
    }
    if source.may_recurse() && data.starts_with(&source.path) {
        return Err(io::Error::other(
            "Cannot Trash an ancestor of its Trash directory",
        ));
    }
    Ok(())
}

fn reserve(entry: &Entry, files: &Path, info: &Path) -> io::Result<(PathBuf, PathBuf)> {
    let mut reservation = tempfile::Builder::new()
        .prefix("tfm-")
        .suffix(".trashinfo")
        .tempfile_in(info)?;
    let stored = reservation
        .path()
        .file_stem()
        .ok_or_else(|| io::Error::other("Invalid Trash reservation"))?
        .to_owned();
    let destination = files.join(stored);
    write_info(reservation.as_file_mut(), &entry.path)?;
    let (_, info_path) = reservation.keep().map_err(io::Error::other)?;
    Ok((destination, info_path))
}

fn private_directory(path: &Path) -> io::Result<()> {
    match DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(io::Error::other(
            "Unsafe Trash directory ownership, type or permissions",
        ));
    }
    Ok(())
}

fn write_info(file: &mut fs::File, path: &Path) -> io::Result<()> {
    let absolute = std::path::absolute(path)?;
    let mut encoded = String::new();
    for byte in absolute.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(byte) {
            encoded.push(char::from(*byte));
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02X}").map_err(io::Error::other)?;
        }
    }
    writeln!(
        file,
        "[Trash Info]\nPath={encoded}\nDeletionDate={}",
        chrono::Local::now().format("%Y-%m-%dT%H:%M:%S")
    )?;
    file.sync_all()
}

fn commit(
    source: &Entry,
    destination: &Path,
    info: &Entry,
    guards: [&Parents; 3],
) -> safety::Result<()> {
    for guard in guards {
        guard.validate()?;
    }
    safety::revalidate(info, true)?;
    safety::revalidate(source, true)?;
    staging::rename_new(&source.path, destination)?;
    let moved = crate::fs::inspect(destination);
    if source.kind != moved.kind
        || source.link_target != moved.link_target
        || source.identity.as_ref().and_then(|id| id.object)
            != moved.identity.as_ref().and_then(|id| id.object)
    {
        return Err(safety::changed(
            "Trash result differs from selected object; reconciliation required",
        ));
    }
    if fs::symlink_metadata(&source.path).is_ok() {
        return Err(safety::changed(
            "Trash source still exists; completion unverified",
        ));
    }
    Ok(())
}

fn cleanup_info(source: &Entry, destination: &Path, info: &Entry) {
    if safety::revalidate(source, true).is_ok()
        && fs::symlink_metadata(destination).is_err()
        && safety::revalidate(info, true).is_ok()
    {
        let _ = fs::remove_file(&info.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_trash_metadata_blocks_source_rename() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("recycled");
        let info = root.path().join("item.trashinfo");
        fs::write(&source, "source preserved").unwrap();
        fs::write(&info, "approved metadata").unwrap();
        let source_entry = crate::fs::inspect(&source);
        let info_entry = crate::fs::inspect(&info);
        let source_guard = Parents::capture(&source).unwrap();
        let target_guard = Parents::capture(&destination).unwrap();
        let info_guard = Parents::capture(&info).unwrap();
        fs::rename(&info, root.path().join("old-info")).unwrap();
        fs::write(&info, "external replacement").unwrap();
        assert!(
            commit(
                &source_entry,
                &destination,
                &info_entry,
                [&source_guard, &target_guard, &info_guard]
            )
            .is_err()
        );
        assert_eq!(fs::read(source).unwrap(), b"source preserved");
        assert_eq!(fs::read(info).unwrap(), b"external replacement");
        assert!(!destination.exists());
    }
}
