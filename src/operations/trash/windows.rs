use super::{
    Receipt,
    acknowledgement::{self, Acknowledgement},
};
use crate::{
    fs::{Entry, EntryKind},
    operations::{
        Outcome,
        safety::{self, Failure, Parents, Result},
    },
};
use std::{
    cell::RefCell, io, os::windows::ffi::OsStrExt, path::Path, rc::Rc, sync::atomic::AtomicBool,
};
use windows::Win32::{
    System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemFree, CoUninitialize,
    },
    UI::Shell::{
        FOF_NO_CONNECTED_ELEMENTS, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING,
        FOFX_ADDUNDORECORD, FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE, FileOperation, IFileOperation,
        IFileOperationProgressSink, IShellItem, SHCreateItemFromParsingName,
        SIGDN_DESKTOPABSOLUTEPARSING,
    },
};
use windows_core::{HRESULT, PCWSTR, Ref};

mod metadata;
mod sink;
#[cfg(test)]
mod tests;

#[derive(Default)]
struct Observed {
    callbacks: Vec<Acknowledgement>,
    refused: Option<String>,
}

pub(super) fn put(entry: &Entry, cancel: &AtomicBool) -> Result<Receipt> {
    std::thread::scope(|scope| scope.spawn(|| recycle_sta(entry, cancel)).join()).unwrap_or_else(
        |_| {
            Err(Failure {
                outcome: Outcome::OutcomeUnknown,
                detail: "Windows Trash STA stopped without acknowledgement; no automatic retry"
                    .into(),
            })
        },
    )
}

struct Apartment;

impl Apartment {
    #[allow(
        unsafe_code,
        reason = "Approved native Trash boundary: initialize one dedicated STA thread"
    )]
    fn initialize() -> io::Result<Self> {
        // SAFETY: this dedicated new thread has no existing COM apartment. Its
        // non-Send interface values are released before Apartment's balanced teardown.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
            .ok()
            .map_err(|error| native_error("Initialize Trash STA", error))?;
        Ok(Self)
    }
}

impl Drop for Apartment {
    #[allow(
        unsafe_code,
        reason = "Approved native Trash boundary: balanced teardown on the initializing thread"
    )]
    fn drop(&mut self) {
        // SAFETY: construction succeeded on this same scoped thread; all COM
        // values were created after this guard and have already been released.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        unsafe { CoUninitialize() };
    }
}

#[allow(
    unsafe_code,
    reason = "Approved native Trash boundary: synchronous Shell calls confined to one owned STA"
)]
fn recycle_sta(entry: &Entry, cancel: &AtomicBool) -> Result<Receipt> {
    let _apartment = Apartment::initialize()?;
    let (parents, item) = preflight(entry)?;
    let observed = Rc::new(RefCell::new(Observed::default()));
    let sink: IFileOperationProgressSink = sink::Sink {
        entry: entry.clone(),
        parents: parents.clone(),
        observed: observed.clone(),
    }
    .into();
    // SAFETY: all interfaces, UTF-16 buffers and callback state live on this STA
    // through PerformOperations. The callback is explicitly non-agile; no COM
    // object or borrowed pointer crosses the scoped thread boundary.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe {
        // SILENT hides progress only; no auto-Yes flag is set. WANTNUKEWARNING
        // is a warning, not proof that Windows can never delete permanently.
        let operation = configured(
            FOFX_RECYCLEONDELETE
                | FOFX_ADDUNDORECORD
                | FOF_WANTNUKEWARNING
                | FOF_NO_CONNECTED_ELEMENTS
                | FOF_SILENT
                | FOF_NOERRORUI
                | FOFX_EARLYFAILURE,
        )?;
        operation
            .DeleteItem(&item, &sink)
            .map_err(|error| native_error("Queue selected Shell item", error))?;
        parents.validate()?;
        safety::revalidate(entry, true)?;
        crate::fs::check_cancelled(cancel)?;
        let performed = operation.PerformOperations();
        let aborted = operation.GetAnyOperationsAborted();
        finish(
            entry,
            &observed.borrow(),
            performed,
            aborted.map(|value| value.as_bool()),
        )
    }
}

fn preflight(entry: &Entry) -> Result<(Parents, IShellItem)> {
    let parents = Parents::capture(&entry.path)?;
    safety::revalidate(entry, true)?;
    if entry.kind == EntryKind::ReparsePoint {
        return Err(io::Error::other("Unsupported reparse object; native Trash refused").into());
    }
    let path = crate::operations::locks::namespace(&entry.path)?;
    Ok((parents, shell_item(&path)?))
}

#[allow(
    unsafe_code,
    reason = "Approved native Trash boundary: configure one owned Shell operation on its STA"
)]
fn configured(flags: windows::Win32::UI::Shell::FILEOPERATION_FLAGS) -> io::Result<IFileOperation> {
    // SAFETY: caller holds a live Apartment on this STA; the returned owned
    // interface is dropped before apartment teardown. Flags never auto-answer Yes.
    unsafe {
        let operation: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| native_error("Create Shell operation", error))?;
        operation
            .SetOperationFlags(flags)
            .map_err(|error| native_error("Set Shell operation flags", error))?;
        Ok(operation)
    }
}

fn finish(
    entry: &Entry,
    observed: &Observed,
    performed: windows_core::Result<()>,
    aborted: windows_core::Result<bool>,
) -> Result<Receipt> {
    let retained = safety::revalidate(entry, true).is_ok();
    if observed.callbacks.is_empty()
        && retained
        && let Some(reason) = &observed.refused
    {
        return Err(safety::changed(reason));
    }
    acknowledgement::classify(
        performed.is_ok(),
        aborted.as_ref().ok().copied(),
        &observed.callbacks,
        retained,
    )
    .map_err(|mut error| {
        for native in [performed.err(), aborted.err()].into_iter().flatten() {
            error.detail.push_str(&format!("; Shell: {native}"));
        }
        error
    })
}

fn native_error(stage: &str, error: windows_core::Error) -> io::Error {
    io::Error::other(format!("{stage}: {error}"))
}

fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::other("NUL in native Shell path"));
    }
    wide.push(0);
    Ok(wide)
}

#[allow(
    unsafe_code,
    reason = "Approved native Trash boundary: Shell borrows live NUL-terminated native UTF-16"
)]
fn shell_item(path: &Path) -> io::Result<IShellItem> {
    let wide = wide(path)?;
    // SAFETY: the buffer is native UTF-16, NUL-terminated and live throughout
    // this synchronous call. Shell returns an owned reference in the current STA.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None) }
        .map_err(|error| native_error("Create Shell item from native path", error))
}

#[allow(
    unsafe_code,
    reason = "Approved native Trash boundary: copy and free a Shell-owned optional parsing name in its STA"
)]
fn display_name(item: &IShellItem, kind: windows::Win32::UI::Shell::SIGDN) -> Option<Vec<u16>> {
    // SAFETY: item is a live borrowed callback reference on its owning STA.
    // GetDisplayName returns CoTaskMem storage; copy before freeing it exactly once.
    unsafe {
        let name = item.GetDisplayName(kind).ok()?;
        if name.is_null() {
            return None;
        }
        let owned = name.as_wide().to_vec();
        CoTaskMemFree(Some(name.0.cast()));
        Some(owned)
    }
}
