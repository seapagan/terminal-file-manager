//! Audited read-only Win32 queries; native UTF-16 never passes through display text.
use std::os::windows::{
    ffi::{OsStrExt, OsStringExt},
    fs::{MetadataExt, OpenOptionsExt},
    io::AsRawHandle,
};
use std::{
    fs::{File, OpenOptions},
    io,
    mem::MaybeUninit,
    path::Path,
};
use windows_sys::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, FileIdBothDirectoryInfo, FindClose, FindFirstFileW, GetDriveTypeW,
        GetFileInformationByHandle, GetFileInformationByHandleEx, GetFinalPathNameByHandleW,
        GetVolumeInformationByHandleW, VOLUME_NAME_GUID, WIN32_FIND_DATAW,
    },
};

/// One synchronous call owns exclusive access to this initialized, aligned buffer.
#[repr(align(8))]
pub(super) struct DirectoryBuffer(pub [u8; 65536]);

pub(super) fn open_directory(path: &Path) -> io::Result<File> {
    let wide = extended_find_path(path)?;
    let native = std::ffi::OsString::from_wide(&wide[..wide.len() - 1]);
    OpenOptions::new()
        .access_mode(FILE_LIST_DIRECTORY)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(Path::new(&native))
}

pub(super) fn unsupported(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(1 | 50 | 87))
}

#[allow(
    unsafe_code,
    reason = "Synchronous read-only volume queries borrow an owned File and initialized bounded UTF-16 outputs"
)]
pub(super) fn query_directory_volume(file: &File) -> io::Result<Option<u64>> {
    let mut serial = 0u32;
    let mut name = [0u16; 261];
    // SAFETY: File owns the live borrowed handle. All supplied output pointers
    // address initialized storage of the exact advertised capacity. Null outputs
    // are optional. Synchronous only; reassess if capacities or ownership change.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    if unsafe {
        GetVolumeInformationByHandleW(
            file.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            &mut serial,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            name.as_mut_ptr(),
            name.len() as u32,
        )
    } == 0
    {
        return volume_error(io::Error::last_os_error(), false);
    }
    if name[..5] != [78, 84, 70, 83, 0] {
        return Ok(None);
    }
    let mut path = vec![0u16; 261];
    loop {
        // SAFETY: File stays live and path exclusively owns initialized UTF-16
        // storage of the checked length passed here. No pointer survives this
        // synchronous call; reassess if resizing or borrow lifetimes change.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        let length = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                path.as_mut_ptr(),
                path.len() as u32,
                VOLUME_NAME_GUID,
            )
        } as usize;
        if length == 0 {
            return volume_error(io::Error::last_os_error(), true);
        }
        if finish_final_path(&mut path, length)? {
            break;
        }
    }
    let Some(root) = super::directory_records::volume_root(&path) else {
        return Ok(None);
    };
    // SAFETY: root owns a validated NUL-terminated native volume-GUID root and
    // remains live through the synchronous call. No null/current-drive operand;
    // reassess if root validation or pointer lifetime changes.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
    Ok((kind == 3).then_some(u64::from(serial)))
}

fn volume_error(error: io::Error, guid_query: bool) -> io::Result<Option<u64>> {
    // A live remote handle has no local volume GUID. Re-enumeration still uses
    // the selected spelling and original before/after directory verification.
    if unsupported(&error) || (guid_query && error.raw_os_error() == Some(3)) {
        Ok(None)
    } else {
        Err(error)
    }
}

fn finish_final_path(path: &mut Vec<u16>, length: usize) -> io::Result<bool> {
    if length < path.len() {
        path.truncate(length);
        return Ok(true);
    }
    let capacity = length
        .checked_add(1)
        .filter(|size| *size <= 65536)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid native final-path length",
            )
        })?;
    path.resize(capacity, 0);
    Ok(false)
}

#[allow(
    unsafe_code,
    reason = "Synchronous enumeration borrows a live File and exclusive initialized 8-byte-aligned 64 KiB storage"
)]
pub(super) fn read_directory_batch(file: &File, buffer: &mut DirectoryBuffer) -> io::Result<bool> {
    buffer.0.fill(0);
    // SAFETY: the owned File remains live, buffer is initialized, aligned to
    // eight bytes and exclusively borrowed for its exact 65536-byte capacity.
    // Decode only checked byte slices after success; no typed record references.
    // Reassess if buffer layout, size or synchronous handle ownership changes.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdBothDirectoryInfo,
            buffer.0.as_mut_ptr().cast(),
            65536,
        )
    } != 0
    {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(18) {
        Ok(false)
    } else {
        Err(error)
    }
}

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
    // The synchronous query succeeded into zero-backed BY_HANDLE_FILE_INFORMATION;
    // the owned File stayed live throughout the call and remains live for this read.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
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
