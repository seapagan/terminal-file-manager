//! Audited read-only Win32 queries; native UTF-16 never passes through display text.
use std::os::windows::{
    ffi::{OsStrExt, OsStringExt},
    fs::{MetadataExt, OpenOptionsExt},
    io::AsRawHandle,
};
use std::{fs::OpenOptions, io, mem::MaybeUninit, path::Path};
use windows_sys::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        FindClose, FindFirstFileW, GetFileInformationByHandle, WIN32_FIND_DATAW,
    },
};

#[allow(
    unsafe_code,
    reason = "Read-only Win32 tag query with initialized output and an owned search handle"
)]
pub(super) fn reparse_tag(path: &Path) -> io::Result<u32> {
    // FindFirstFile cannot query a drive/share root. Ordinary roots need no tag.
    if std::fs::symlink_metadata(path)?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0 {
        return Ok(0);
    }
    struct Search(HANDLE);
    impl Drop for Search {
        fn drop(&mut self) {
            // SAFETY: Search owns one successful FindFirstFile handle; no other
            // code closes it. Drop runs on every return and during unwinding.
            // Audited cleanup; reassess if search-handle ownership changes.
            // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
            unsafe {
                FindClose(self.0);
            }
        }
    }
    let wide = extended_find_path(path)?;
    let mut data = MaybeUninit::<WIN32_FIND_DATAW>::zeroed();
    // SAFETY: wide is NUL-terminated native UTF-16. Output has the exact SDK
    // layout, writable storage and lifetime required by this synchronous call.
    // Audited read-only query; reassess if buffer or path lifetimes change.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let handle = unsafe { FindFirstFileW(wide.as_ptr(), data.as_mut_ptr()) };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let _search = Search(handle);
    // SAFETY: success initialized this zero-backed SDK structure.
    // Audited success-only read; reassess if SDK output initialization changes.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let data = unsafe { data.assume_init() };
    Ok(
        if data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            data.dwReserved0
        } else {
            0
        },
    )
}

#[allow(
    unsafe_code,
    reason = "Read-only no-follow identity query with a borrowed File handle and initialized output"
)]
pub(super) fn object_id(path: &Path) -> io::Result<(u64, u64)> {
    let wide = extended_find_path(path)?;
    let native = std::ffi::OsString::from_wide(&wide[..wide.len() - 1]);
    let file = OpenOptions::new()
        .access_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(Path::new(&native))?;
    let mut data = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    // SAFETY: the File's borrowed handle remains valid through this synchronous
    // read; output storage has the exact SDK structure size and lifetime.
    // Audited read-only query; reassess if the borrowed handle lifetime changes.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), data.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: success initialized this zero-backed SDK structure.
    let data = unsafe { data.assume_init() };
    let index = (u64::from(data.nFileIndexHigh) << 32) | u64::from(data.nFileIndexLow);
    Ok((u64::from(data.dwVolumeSerialNumber), index))
}

fn extended_find_path(path: &Path) -> io::Result<Vec<u16>> {
    let absolute = std::path::absolute(path)?;
    let mut native: Vec<u16> = absolute.as_os_str().encode_wide().collect();
    if native.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NUL in native path",
        ));
    }
    if absolute.file_name().is_some() {
        while native.last().is_some_and(|c| *c == 92 || *c == 47) {
            native.pop();
        }
    }
    let mut wide: Vec<u16> = if native.starts_with(&[92, 92, 63, 92]) {
        native
    } else if native.starts_with(&[92, 92]) {
        "\\\\?\\UNC\\"
            .encode_utf16()
            .chain(native[2..].iter().copied())
            .collect()
    } else {
        "\\\\?\\".encode_utf16().chain(native).collect()
    };
    wide.push(0);
    Ok(wide)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, fs, path::PathBuf};

    #[test]
    fn native_path_normalization_retains_roots_unc_and_unpaired_utf16() {
        for (input, expected) in [
            ("C:\\folder\\", "\\\\?\\C:\\folder"),
            ("\\\\?\\C:\\folder\\", "\\\\?\\C:\\folder"),
            (
                "\\\\server\\share\\folder\\",
                "\\\\?\\UNC\\server\\share\\folder",
            ),
            (
                "\\\\?\\UNC\\server\\share\\folder\\",
                "\\\\?\\UNC\\server\\share\\folder",
            ),
            ("C:\\", "\\\\?\\C:\\"),
        ] {
            let wide = extended_find_path(Path::new(input)).unwrap();
            assert_eq!(wide, expected.encode_utf16().chain([0]).collect::<Vec<_>>());
        }
        let mut native: Vec<_> = "C:\\native-".encode_utf16().collect();
        native.push(0xD800);
        let path = PathBuf::from(OsString::from_wide(&native));
        let wide = extended_find_path(&path).unwrap();
        assert_eq!(&wide[4..wide.len() - 1], native);
    }

    #[test]
    fn nonreparse_entries_and_failed_native_queries_remain_explicit() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("ordinary");
        fs::write(&file, "data").unwrap();
        assert_eq!(reparse_tag(&file).unwrap(), 0);
        assert_eq!(reparse_tag(root.path()).unwrap(), 0);
        assert!(object_id(&file).is_ok());
        for path in [root.path().join("missing"), file.join("not-a-directory")] {
            assert!(reparse_tag(&path).is_err());
            assert!(object_id(&path).is_err());
        }
        let invalid = PathBuf::from(OsString::from_wide(&[67, 58, 92, 0]));
        assert_eq!(
            object_id(&invalid).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn nofollow_identity_distinguishes_retained_replaced_objects_and_links() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("file");
        let link = root.path().join("link");
        fs::write(&file, "same").unwrap();
        std::os::windows::fs::symlink_file(&file, &link).unwrap();
        assert_eq!(reparse_tag(&link).unwrap(), 0xA000_000C);
        let mut trailing = link.as_os_str().to_os_string();
        trailing.push("\\");
        // A file symlink with a trailing separator is an invalid directory path.
        assert!(reparse_tag(Path::new(&trailing)).is_err());
        let original = object_id(&file).unwrap();
        let link_id = object_id(&link).unwrap();
        assert_ne!(original, link_id);
        fs::rename(&file, root.path().join("retained-original")).unwrap();
        fs::write(&file, "same").unwrap();
        assert_ne!(original, object_id(&file).unwrap());
        assert_eq!(link_id, object_id(&link).unwrap());
        fs::remove_file(&file).unwrap();
        assert_eq!(reparse_tag(&link).unwrap(), 0xA000_000C);
        assert_eq!(link_id, object_id(&link).unwrap());
    }
}
