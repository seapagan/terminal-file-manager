use super::Receipt;
use crate::{
    fs::Entry,
    operations::{
        Outcome,
        safety::{self, Failure, Parents, Result},
    },
};
use objc2::rc::{Retained, autoreleasepool};
use objc2_foundation::{NSFileManager, NSURL};
use std::{
    ffi::{CStr, CString, OsString},
    io,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::atomic::AtomicBool,
};

pub(super) fn put(entry: &Entry, cancel: &AtomicBool) -> Result<Receipt> {
    autoreleasepool(|_| {
        let parents = Parents::capture(&entry.path)?;
        let path = crate::operations::locks::namespace(&entry.path)?;
        let url = file_url(&path, entry.may_recurse())?;
        parents.validate()?;
        safety::revalidate(entry, true)?;
        crate::fs::check_cancelled(cancel)?;
        let mut resulting = None;
        if let Err(error) = NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting))
        {
            return Err(Failure {
                outcome: if safety::revalidate(entry, true).is_ok() {
                    Outcome::Failed
                } else {
                    Outcome::OutcomeUnknown
                },
                detail: format!(
                    "Native macOS Trash: {} (code {})",
                    error.localizedDescription(),
                    error.code()
                ),
            });
        }
        acknowledged(entry, resulting.as_deref().and_then(filesystem_path))
    })
}

fn acknowledged(entry: &Entry, path: Option<PathBuf>) -> Result<Receipt> {
    if std::fs::symlink_metadata(&entry.path).is_ok() {
        return Err(unknown(
            "macOS acknowledged Trash but source remains; reconciliation required",
        ));
    }
    if let Some(path) = &path {
        let moved = crate::fs::inspect(path);
        if moved.kind != entry.kind
            || moved.link_target != entry.link_target
            || moved.identity.as_ref().and_then(|id| id.object)
                != entry.identity.as_ref().and_then(|id| id.object)
        {
            return Err(unknown(
                "macOS returned a different or unverified Trash object; no automatic retry",
            ));
        }
    }
    Ok(Receipt::Mac { path })
}

fn unknown(detail: &str) -> Failure {
    Failure {
        outcome: Outcome::OutcomeUnknown,
        detail: detail.into(),
    }
}

#[allow(
    unsafe_code,
    reason = "Approved native Trash boundary: live NUL-terminated native bytes become an owned NSURL"
)]
fn file_url(path: &Path, directory: bool) -> io::Result<Retained<NSURL>> {
    let bytes = CString::new(path.as_os_str().as_bytes())?;
    let pointer = NonNull::new(bytes.as_ptr().cast_mut())
        .ok_or_else(|| io::Error::other("Missing native path"))?;
    // SAFETY: bytes is a live NUL-terminated native filesystem representation
    // throughout the synchronous call; NSURL owns its result, with no final-link resolution.
    Ok(unsafe {
        NSURL::fileURLWithFileSystemRepresentation_isDirectory_relativeToURL(
            pointer, directory, None,
        )
    })
}

#[allow(
    unsafe_code,
    reason = "Approved native Trash boundary: copy URL-owned filesystem bytes while its NSURL is live"
)]
fn filesystem_path(url: &NSURL) -> Option<PathBuf> {
    if !url.isFileURL() {
        return None;
    }
    // SAFETY: Foundation returns a NUL-terminated filesystem representation whose
    // storage remains owned by the borrowed live NSURL; bytes are copied before return.
    let bytes = unsafe { CStr::from_ptr(url.fileSystemRepresentation().as_ptr()) }
        .to_bytes()
        .to_vec();
    Some(PathBuf::from(OsString::from_vec(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_optional_url_succeeds_only_after_native_acknowledgement_and_source_removal() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("file");
        std::fs::write(&source, "owned").unwrap();
        let entry = crate::fs::inspect(&source);
        assert!(acknowledged(&entry, None).is_err());
        std::fs::rename(&source, root.path().join("moved")).unwrap();
        assert!(matches!(
            acknowledged(&entry, None).unwrap(),
            Receipt::Mac { path: None }
        ));
        assert!(acknowledged(&entry, Some(root.path().join("missing"))).is_err());
    }

    #[test]
    fn foundation_trashes_owned_files_collisions_and_symlinks_without_following_targets() {
        if std::env::var("RUNNER_ENVIRONMENT").as_deref() != Ok("github-hosted") {
            eprintln!("NOT RUN: native macOS Trash requires an ephemeral hosted identity");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("same-雪-\n");
        let target = root.path().join("sentinel");
        std::fs::write(&target, "preserved").unwrap();
        std::fs::write(&source, "first").unwrap();
        let first = put(&crate::fs::inspect(&source), &false.into()).unwrap();
        std::fs::write(&source, "second").unwrap();
        let second = put(&crate::fs::inspect(&source), &false.into()).unwrap();
        assert_ne!(first.path(), second.path());
        assert_eq!(std::fs::read(first.path().unwrap()).unwrap(), b"first");
        assert_eq!(std::fs::read(second.path().unwrap()).unwrap(), b"second");
        std::fs::rename(first.path().unwrap(), root.path().join("restored-first")).unwrap();
        std::fs::rename(second.path().unwrap(), &source).unwrap();
        std::fs::remove_file(&source).unwrap();
        std::os::unix::fs::symlink(&target, &source).unwrap();
        let receipt = put(&crate::fs::inspect(&source), &false.into()).unwrap();
        assert_eq!(std::fs::read_link(receipt.path().unwrap()).unwrap(), target);
        assert_eq!(std::fs::read(&target).unwrap(), b"preserved");
        std::fs::rename(receipt.path().unwrap(), &source).unwrap();
    }

    #[test]
    fn native_url_roundtrip_preserves_control_and_non_ascii_path_bytes() {
        autoreleasepool(|_| {
            let path = PathBuf::from("/tmp/tfm-雪-%-\n");
            assert_eq!(
                filesystem_path(&file_url(&path, false).unwrap()).unwrap(),
                path
            );
        });
    }
}
