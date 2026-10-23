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
        let error = std::io::Error::last_os_error();
        if error.raw_os_error().is_some_and(|code| {
            [nix::libc::ENOSYS, nix::libc::EINVAL, nix::libc::EOPNOTSUPP].contains(&code)
        }) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                format!("Native exclusive rename unavailable; no fallback: {error}"),
            ));
        }
        Err(error)
    }
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
