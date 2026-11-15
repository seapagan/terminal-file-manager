//! Synchronous read-only FFI boundary, called only on ownership worker threads.
use super::Name;
use crate::fs::Identity;
use std::{
    fs::OpenOptions,
    io,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
    ptr,
};
use windows_sys::Win32::{
    Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_NONE_MAPPED, LocalFree},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSidToSidW, GetSecurityInfo, SE_FILE_OBJECT,
        },
        LookupAccountSidW, OWNER_SECURITY_INFORMATION,
    },
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
        READ_CONTROL,
    },
};

struct Local(*mut std::ffi::c_void);
impl Drop for Local {
    fn drop(&mut self) {
        // SAFETY: Local owns an allocation returned by a successful Win32
        // security conversion/query, never a borrowed SID within that allocation.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        unsafe {
            LocalFree(self.0);
        }
    }
}

/// Read a no-follow object's SID, refusing identity changes since enumeration.
///
/// # Errors
/// Reports access, disappearance, replacement and security descriptor errors.
pub fn read_sid(path: &Path, expected: &Identity) -> io::Result<String> {
    let file = OpenOptions::new()
        .access_mode(READ_CONTROL)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(path)?;
    verify_identity(&file, expected)?;
    let mut owner = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    // SAFETY: file retains the handle; all outputs are initialized pointers.
    // Only owner information is requested. The returned SID is borrowed from
    // descriptor and consumed before descriptor's Local allocation is released.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _descriptor = Local(descriptor);
    if owner.is_null() {
        return Err(io::Error::other("Security descriptor has no owner"));
    }
    let mut string = ptr::null_mut();
    // SAFETY: owner remains valid inside descriptor, and string is writable.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    if unsafe { ConvertSidToStringSidW(owner, &mut string) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _string = Local(string.cast());
    // A SID string is bounded by the native SID structure (15 subauthorities).
    let mut length = 0;
    // SAFETY: successful conversion returns a NUL-terminated UTF-16 allocation;
    // the Local guard keeps it live throughout the scan and copy.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe {
        while *string.add(length) != 0 {
            length += 1;
        }
        Ok(String::from_utf16_lossy(std::slice::from_raw_parts(
            string, length,
        )))
    }
}

fn verify_identity(file: &std::fs::File, expected: &Identity) -> io::Result<()> {
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    // SAFETY: file retains its handle; info provides the exact writable SDK layout.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful query initialized the zero-backed structure.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let info = unsafe { info.assume_init() };
    let object = (
        u64::from(info.dwVolumeSerialNumber),
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    );
    if expected.object != Some(object) || file.metadata()?.created().ok() != expected.created {
        return Err(io::Error::other("Owner object changed since enumeration"));
    }
    Ok(())
}

/// Translate a SID independently of filesystem access; preserve unmapped status.
pub fn resolve_name(sid: &str) -> Name {
    match account_name(sid) {
        Ok(Some(name)) => Name::Resolved(name),
        Ok(None) => Name::Unmapped,
        Err(error) => Name::Failed(error.to_string()),
    }
}

fn account_name(sid: &str) -> io::Result<Option<String>> {
    if sid.contains('\0') {
        return Err(io::Error::other("NUL in SID"));
    }
    let wide: Vec<_> = sid.encode_utf16().chain([0]).collect();
    let mut native = ptr::null_mut();
    // SAFETY: input is NUL-terminated and lives through the synchronous call.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    if unsafe { ConvertStringSidToSidW(wide.as_ptr(), &mut native) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _native = Local(native);
    let (mut names, mut domains, mut kind) = (0, 0, 0);
    // SAFETY: native is a valid owned SID; zero sizes request output lengths.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let ok = unsafe {
        LookupAccountSidW(
            ptr::null(),
            native,
            ptr::null_mut(),
            &mut names,
            ptr::null_mut(),
            &mut domains,
            &mut kind,
        )
    };
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(ERROR_NONE_MAPPED as i32) {
        return Ok(None);
    }
    if ok == 0 && error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
        return Err(error);
    }
    lookup(native, names, domains, &mut kind)
}

fn lookup(
    sid: *mut std::ffi::c_void,
    mut names: u32,
    mut domains: u32,
    kind: &mut i32,
) -> io::Result<Option<String>> {
    // A changing account record may outgrow the first sizing query. Retry only
    // boundedly and reject implausible output sizes before allocating.
    for _ in 0..3 {
        if names > 32768 || domains > 32768 {
            return Err(io::Error::other("Account name too long"));
        }
        let mut name = vec![0u16; names as usize];
        let mut domain = vec![0u16; domains as usize];
        // SAFETY: SID's Local guard in account_name outlives this call; buffers
        // have exactly the declared capacities, and kind is writable SDK output.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        if unsafe {
            LookupAccountSidW(
                ptr::null(),
                sid,
                name.as_mut_ptr(),
                &mut names,
                domain.as_mut_ptr(),
                &mut domains,
                kind,
            )
        } != 0
        {
            let name = String::from_utf16_lossy(&name[..names as usize]);
            let domain = String::from_utf16_lossy(&domain[..domains as usize]);
            return Ok(Some(if domain.is_empty() {
                name
            } else {
                format!("{domain}\\{name}")
            }));
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error().map(|code| code as u32) {
            Some(ERROR_NONE_MAPPED) => return Ok(None),
            Some(ERROR_INSUFFICIENT_BUFFER) => continue,
            _ => return Err(error),
        }
    }
    Err(io::Error::other("Account name changed during lookup"))
}
