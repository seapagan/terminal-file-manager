use super::*;
use crate::operations::trash::ShellMetadata;
use windows::Win32::UI::Shell::{
    IShellItem2, PropertiesSystem::PSGetPropertyKeyFromName, SIGDN_PARENTRELATIVEEDITING,
};
use windows_core::{Interface, PWSTR, w};

pub(super) fn extract(item: &IShellItem) -> ShellMetadata {
    let properties = item.cast::<IShellItem2>().ok();
    ShellMetadata {
        display_name: display_name(item, SIGDN_PARENTRELATIVEEDITING),
        deleted_from: properties.as_ref().and_then(deleted_from),
        deletion_time: properties.as_ref().and_then(deletion_time),
    }
}

#[allow(
    unsafe_code,
    reason = "Approved native Trash boundary: optional properties read from the live recycled Shell item"
)]
fn deleted_from(item: &IShellItem2) -> Option<Vec<u16>> {
    // SAFETY: canonical property name is static NUL-terminated UTF-16; item is
    // live on this STA. The returned COM allocation is copied and freed below.
    unsafe {
        let key = property_key(w!("System.Recycle.DeletedFrom"))?;
        owned_string(item.GetString(&key).ok()?)
    }
}

#[allow(
    unsafe_code,
    reason = "Approved native Trash boundary: optional native deletion time copied while the Shell item is live"
)]
fn deletion_time(item: &IShellItem2) -> Option<u64> {
    // SAFETY: static canonical name and stack property key live for these
    // synchronous STA calls; FILETIME is returned by value with no borrowed data.
    unsafe {
        let key = property_key(w!("System.Recycle.DateDeleted"))?;
        let time = item.GetFileTime(&key).ok()?;
        Some((u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime))
    }
}

#[allow(
    unsafe_code,
    reason = "Approved native Trash boundary: copy and release a property-system owned string"
)]
fn owned_string(value: PWSTR) -> Option<Vec<u16>> {
    if value.is_null() {
        return None;
    }
    // SAFETY: successful GetString returned NUL-terminated CoTaskMem storage;
    // copy the native UTF-16 before freeing exactly this allocation once.
    unsafe {
        let owned = value.as_wide().to_vec();
        CoTaskMemFree(Some(value.0.cast()));
        Some(owned)
    }
}

#[allow(
    unsafe_code,
    reason = "Approved native Trash boundary: synchronous property-name lookup into initialized storage"
)]
fn property_key(name: PCWSTR) -> Option<windows::Win32::Foundation::PROPERTYKEY> {
    let mut key = windows::Win32::Foundation::PROPERTYKEY::default();
    // SAFETY: callers pass static NUL-terminated property names; key is live,
    // initialized writable SDK storage and is read only after API success.
    unsafe { PSGetPropertyKeyFromName(name, &mut key) }.ok()?;
    Some(key)
}
