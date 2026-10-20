use super::LinuxReceipt as Receipt;
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

/// Freedesktop home/per-volume Trash with no copy/delete fallback.
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
        self.put_checked(entry, &false.into())
            .map_err(|failure| io::Error::other(failure.detail))
    }

    pub(super) fn put_checked(
        &self,
        entry: &Entry,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> safety::Result<Receipt> {
        let source_guard = Parents::capture(&entry.path)?;
        safety::revalidate(entry, true)?;
        validate_scope(entry, cancel)?;
        let mounts = fs::read("/proc/self/mountinfo")?;
        let physical = crate::operations::locks::namespace(&entry.path)?;
        let (files, info, original) = self.prepare(entry, &physical)?;
        let (destination, info_path) = reserve(&original, &files, &info)?;
        if let Err(error) = crate::fs::check_cancelled(cancel) {
            cleanup_info(entry, &destination, &crate::fs::inspect(&info_path));
            return Err(error.into());
        }
        finish(entry, destination, info_path, source_guard, &mounts)
    }

    fn prepare(
        &self,
        source: &Entry,
        physical: &Path,
    ) -> safety::Result<(PathBuf, PathBuf, PathBuf)> {
        let device = fs::symlink_metadata(physical)?.dev();
        let (trash, original) = self.location(source, physical, device)?;
        let (files, info) = prepare_store(&trash, device)?;
        safety::reject_overlap(source, &files.join("reservation"))?;
        Ok((files, info, original))
    }
    fn location(
        &self,
        source: &Entry,
        physical: &Path,
        device: u64,
    ) -> safety::Result<(PathBuf, PathBuf)> {
        validate_data(&self.data, source)?;
        let (trash, original) = if super::volume::existing_device(&self.data)? == device {
            fs::create_dir_all(&self.data)?;
            (self.data.join("Trash"), physical.to_owned())
        } else {
            let top = super::volume::topdir(physical)?;
            (
                volume_trash(&top, device)?,
                physical
                    .strip_prefix(&top)
                    .map_err(io::Error::other)?
                    .to_owned(),
            )
        };
        Ok((trash, original))
    }
}

fn finish(
    entry: &Entry,
    destination: PathBuf,
    info_path: PathBuf,
    source_guard: Parents,
    mounts: &[u8],
) -> safety::Result<Receipt> {
    let info_entry = crate::fs::inspect(&info_path);
    let target_guard = Parents::capture(&destination)?;
    let info_guard = Parents::capture(&info_path)?;
    let result = if fs::read("/proc/self/mountinfo")? != mounts {
        Err(safety::changed("Mount namespace changed before Trash"))
    } else {
        commit(
            entry,
            &destination,
            &info_entry,
            [&source_guard, &target_guard, &info_guard],
        )
    };
    if let Err(error) = result {
        cleanup_info(entry, &destination, &info_entry);
        return Err(submission_failure(entry, error));
    }
    Ok(Receipt {
        path: destination,
        info: info_path,
    })
}

fn submission_failure(entry: &Entry, error: safety::Failure) -> safety::Failure {
    if safety::revalidate(entry, true).is_ok() {
        error
    } else {
        safety::Failure {
            outcome: crate::operations::Outcome::OutcomeUnknown,
            detail: format!(
                "Trash submission unacknowledged; reconciliation required: {}",
                error.detail
            ),
        }
    }
}

fn prepare_store(trash: &Path, device: u64) -> io::Result<(PathBuf, PathBuf)> {
    private_directory(trash)?;
    let files = trash.join("files");
    let info = trash.join("info");
    private_directory(&files)?;
    private_directory(&info)?;
    let mounts = crate::operations::mounts::Mounts::capture(crate::operations::Operation::Trash)?;
    for path in [trash, &files, &info] {
        if fs::symlink_metadata(path)?.dev() != device {
            return Err(io::Error::other("Trash storage crosses a device; refused"));
        }
        mounts
            .reject(&crate::fs::inspect(path))
            .map_err(|error| io::Error::other(error.detail))?;
    }
    Ok((files, info))
}

fn volume_trash(top: &Path, device: u64) -> io::Result<PathBuf> {
    let uid = nix::unistd::geteuid();
    let shared = top.join(".Trash");
    let preferred = shared.join(uid.to_string());
    if valid_shared(&shared, device) && prepare_store(&preferred, device).is_ok() {
        return Ok(preferred);
    }
    let fallback = top.join(format!(".Trash-{uid}"));
    prepare_store(&fallback, device)?;
    Ok(fallback)
}

fn valid_shared(path: &Path, device: u64) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.dev() == device
            && metadata.mode() & 0o1000 != 0
            && [0, nix::unistd::geteuid().as_raw()].contains(&metadata.uid())
    })
}

fn validate_scope(entry: &Entry, cancel: &std::sync::atomic::AtomicBool) -> safety::Result<()> {
    let plan = crate::operations::Plan::paths(
        crate::operations::Operation::Trash,
        vec![entry.path.clone()],
        None,
    )?;
    crate::operations::Preview::capture(&plan, cancel)?;
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

fn reserve(original: &Path, files: &Path, info: &Path) -> io::Result<(PathBuf, PathBuf)> {
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
    write_info(reservation.as_file_mut(), original)?;
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
    let mut encoded = String::new();
    for byte in path.as_os_str().as_bytes() {
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
    fn volume_storage_prefers_sticky_shared_and_falls_back_from_unsafe_components() {
        let root = tempfile::tempdir().unwrap();
        let device = root.path().metadata().unwrap().dev();
        let uid = nix::unistd::geteuid();
        let shared = root.path().join(".Trash");
        fs::create_dir(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o1777)).unwrap();
        let selected = volume_trash(root.path(), device).unwrap();
        assert_eq!(selected, shared.join(uid.to_string()));
        assert_eq!(selected.metadata().unwrap().mode() & 0o777, 0o700);
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o777)).unwrap();
        let fallback = root.path().join(format!(".Trash-{uid}"));
        assert_eq!(volume_trash(root.path(), device).unwrap(), fallback);
        fs::remove_dir(fallback.join("info")).unwrap();
        std::os::unix::fs::symlink(root.path(), fallback.join("info")).unwrap();
        assert!(volume_trash(root.path(), device).is_err());
        assert!(fallback.join("info").is_symlink());
    }

    #[test]
    fn trashinfo_preserves_native_relative_bytes_and_shared_symlinks_are_refused() {
        use std::os::unix::ffi::OsStringExt;
        let root = tempfile::tempdir().unwrap();
        let mut file = tempfile::tempfile().unwrap();
        let path = PathBuf::from(std::ffi::OsString::from_vec(b"dir/a%\xff\n".to_vec()));
        write_info(&mut file, &path).unwrap();
        use std::io::{Read, Seek};
        file.rewind().unwrap();
        let mut text = String::new();
        file.read_to_string(&mut text).unwrap();
        assert!(text.contains("Path=dir/a%25%FF%0A\nDeletionDate="));
        let shared = root.path().join(".Trash");
        std::os::unix::fs::symlink(root.path(), &shared).unwrap();
        assert!(!valid_shared(
            &shared,
            root.path().metadata().unwrap().dev()
        ));
    }

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
