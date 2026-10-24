#[cfg(unix)]
#[allow(
    unsafe_code,
    reason = "Native exclusive rename: borrowed NUL-terminated paths remain live for the synchronous call"
)]
pub(in crate::operations) fn rename_new(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let source = CString::new(source.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // SAFETY: both native byte buffers are NUL-terminated and live for this
    // synchronous syscall. Only exclusive rename is requested; errors have no fallback.
    #[cfg(target_os = "linux")]
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let status = unsafe {
        nix::libc::syscall(
            nix::libc::SYS_renameat2,
            nix::libc::AT_FDCWD,
            source.as_ptr(),
            nix::libc::AT_FDCWD,
            destination.as_ptr(),
            nix::libc::RENAME_NOREPLACE,
        )
    };
    // SAFETY: same buffer/lifetime contract; RENAME_EXCL refuses existing names.
    #[cfg(target_os = "macos")]
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let status = unsafe {
        nix::libc::renamex_np(
            source.as_ptr(),
            destination.as_ptr(),
            nix::libc::RENAME_EXCL,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(classify(std::io::Error::last_os_error()))
    }
}

#[cfg(unix)]
fn classify(error: std::io::Error) -> std::io::Error {
    if error.raw_os_error().is_some_and(|code| {
        [nix::libc::ENOSYS, nix::libc::ENOTSUP, nix::libc::EOPNOTSUPP].contains(&code)
    }) {
        return std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            format!("Native exclusive rename unavailable; no fallback: {error}"),
        );
    }
    // EINVAL also describes invalid paths; it does not establish a missing primitive.
    error
}

#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "Native no-replace rename: borrowed UTF-16 buffers remain live for MoveFileExW with no fallback flags"
)]
pub(in crate::operations) fn rename_new(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    fn wide(path: &std::path::Path) -> std::io::Result<Vec<u16>> {
        let mut value: Vec<_> = path.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "NUL in rename path",
            ));
        }
        value.push(0);
        Ok(value)
    }
    let source = wide(source)?;
    let destination = wide(destination)?;
    // SAFETY: both buffers are NUL-terminated native UTF-16 and remain live for
    // the call. Zero flags prohibit replacement and cross-volume copy/delete.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            0,
        )
    } != 0
    {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn invalid_descendant_rename_keeps_native_einval_and_preserves_the_tree() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        std::fs::create_dir_all(source.join("child")).unwrap();
        std::fs::write(source.join("sentinel"), "retained").unwrap();
        let error = rename_new(&source, &source.join("child/new")).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(nix::libc::EINVAL));
        assert_eq!(std::fs::read(source.join("sentinel")).unwrap(), b"retained");
        assert!(!source.join("child/new").exists());
    }

    #[test]
    fn only_documented_unsupported_errnos_lose_the_native_error_kind() {
        use std::io;
        for code in [nix::libc::ENOSYS, nix::libc::ENOTSUP, nix::libc::EOPNOTSUPP] {
            let error = classify(io::Error::from_raw_os_error(code));
            assert_eq!(error.kind(), io::ErrorKind::Unsupported);
            assert!(error.to_string().contains("no fallback"));
        }
        for code in [
            nix::libc::EINVAL,
            nix::libc::EACCES,
            nix::libc::EXDEV,
            nix::libc::EEXIST,
        ] {
            let error = classify(io::Error::from_raw_os_error(code));
            assert_eq!(error.raw_os_error(), Some(code));
            assert_eq!(error.kind(), io::Error::from_raw_os_error(code).kind());
        }
    }
}
