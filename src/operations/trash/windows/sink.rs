use super::*;
use windows::Win32::UI::Shell::IFileOperationProgressSink_Impl;
use windows_core::{Error, implement};

// SAFETY: only binding-generated COM ownership/vtable code is allowed here;
// the non-agile callback stays on its creating STA, and owns its Rust state.
#[allow(
    unsafe_code,
    reason = "Approved native Trash callback boundary: binding-generated COM vtable and reference counting only"
)]
mod interface {
    use super::*;
    #[implement(IFileOperationProgressSink, Agile = false)]
    pub(in crate::operations::trash::windows) struct Sink {
        pub(in crate::operations::trash::windows) entry: Entry,
        pub(in crate::operations::trash::windows) parents: Parents,
        pub(in crate::operations::trash::windows) observed: Rc<RefCell<Observed>>,
    }
}
pub(super) use interface::Sink;

impl IFileOperationProgressSink_Impl for interface::Sink_Impl {
    fn StartOperations(&self) -> windows_core::Result<()> {
        Ok(())
    }
    fn FinishOperations(&self, _hrresult: HRESULT) -> windows_core::Result<()> {
        Ok(())
    }
    fn PreRenameItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psznewname: &PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PostRenameItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psznewname: &PCWSTR,
        _hrrename: HRESULT,
        _psinewlycreated: Ref<IShellItem>,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PreMoveItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PostMoveItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
        _hrmove: HRESULT,
        _psinewlycreated: Ref<IShellItem>,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PreCopyItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PostCopyItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
        _hrcopy: HRESULT,
        _psinewlycreated: Ref<IShellItem>,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PreDeleteItem(&self, _dwflags: u32, _psiitem: Ref<IShellItem>) -> windows_core::Result<()> {
        let checked = self
            .parents
            .validate()
            .and_then(|()| safety::revalidate(&self.entry, true));
        if let Err(error) = checked {
            self.observed.borrow_mut().refused = Some(error);
            return Err(Error::from_hresult(HRESULT(0x80004004_u32 as i32)));
        }
        Ok(())
    }
    fn PostDeleteItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        hrdelete: HRESULT,
        psinewlycreated: Ref<IShellItem>,
    ) -> windows_core::Result<()> {
        let recycled = psinewlycreated.as_ref();
        let acknowledgement = Acknowledgement {
            status: hrdelete.0,
            error: hrdelete.ok().err().map(|error| error.to_string()),
            recycled: recycled.is_some(),
            parsing_name: recycled
                .and_then(|item| display_name(item, SIGDN_DESKTOPABSOLUTEPARSING)),
            metadata: recycled.map(metadata::extract).unwrap_or_default(),
        };
        self.observed.borrow_mut().callbacks.push(acknowledgement);
        Ok(())
    }
    fn PreNewItem(
        &self,
        _dwflags: u32,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PostNewItem(
        &self,
        _dwflags: u32,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
        _psztemplatename: &PCWSTR,
        _dwfileattributes: u32,
        _hrnew: HRESULT,
        _psinewitem: Ref<IShellItem>,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn UpdateProgress(&self, _iworktotal: u32, _iworksofar: u32) -> windows_core::Result<()> {
        Ok(())
    }
    fn ResetTimer(&self) -> windows_core::Result<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> windows_core::Result<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> windows_core::Result<()> {
        Ok(())
    }
}
