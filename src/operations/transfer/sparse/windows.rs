use std::{fs::File, io, os::windows::io::AsRawHandle};

#[allow(
    unsafe_code,
    reason = "Native sparse stage boundary: synchronously mark only the borrowed owned file handle, without clearing sparse state"
)]
pub(super) fn mark_sparse(file: &File) -> io::Result<()> {
    let mut returned = 0;
    // SAFETY: File owns a live synchronous handle throughout the call. Null
    // buffers request only setting sparse state, never deallocation or clearing.
    let success = unsafe {
        windows_sys::Win32::System::IO::DeviceIoControl(
            file.as_raw_handle(),
            0x0009_00c4,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if success == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[allow(
    unsafe_code,
    reason = "Native sparse allocation evidence: query a live borrowed handle into an initialized, correctly sized FILE_STANDARD_INFO buffer"
)]
pub(super) fn allocated(file: &File) -> io::Result<u64> {
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_STANDARD_INFO, FileStandardInfo, GetFileInformationByHandleEx,
    };
    let mut info = FILE_STANDARD_INFO::default();
    // SAFETY: the handle remains owned by File. The initialized output buffer
    // has the exact documented structure and size and lives through this call.
    let success = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileStandardInfo,
            std::ptr::from_mut(&mut info).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    };
    if success == 0 {
        return Err(io::Error::last_os_error());
    }
    u64::try_from(info.AllocationSize).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::{self, Decision, Event, Operation, Options, Outcome, Plan, Stage};
    use std::{
        fs,
        io::{Read, Seek, SeekFrom, Write},
        sync::atomic::{AtomicBool, Ordering},
    };

    fn fixture() -> (tempfile::TempDir, Plan, Vec<u8>) {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("sparse");
        let mut file = File::options()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&source)
            .unwrap();
        mark_sparse(&file).unwrap();
        file.set_len(8 * 1024 * 1024).unwrap();
        file.write_all(b"head").unwrap();
        file.seek(SeekFrom::End(-4)).unwrap();
        file.write_all(b"tail").unwrap();
        file.sync_all().unwrap();
        assert!(allocated(&file).unwrap() < file.metadata().unwrap().len() / 4);
        drop(file);
        fs::create_dir(root.path().join("target")).unwrap();
        fs::write(root.path().join("target/sparse"), "previous").unwrap();
        let bytes = fs::read(&source).unwrap();
        let plan = Plan::paths(
            Operation::Move,
            vec![source],
            Some(root.path().join("target")),
        )
        .unwrap();
        (root, plan, bytes)
    }

    fn dense_baseline(source: &std::path::Path, destination: &std::path::Path) -> u64 {
        let mut input = File::open(source).unwrap();
        let mut output = File::create(destination).unwrap();
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let count = input.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count]).unwrap();
        }
        output.sync_all().unwrap();
        allocated(&output).unwrap()
    }

    #[test]
    fn ntfs_sparse_copy_and_move_preserve_allocation_and_complete_logical_bytes() {
        for operation in [Operation::Copy, Operation::Move] {
            let (root, mut plan, expected) = fixture();
            plan.operation = operation;
            let source = root.path().join("sparse");
            let dense = dense_baseline(&source, &root.path().join("old-dense-transfer"));
            assert!(
                dense >= expected.len() as u64 / 2,
                "baseline must reproduce expansion"
            );
            let record =
                operations::execute(&plan, Options { checksum: true }, &false.into(), |_| {
                    Decision::Replace
                });
            assert_eq!(record.entries[0].outcome, Outcome::Completed, "{record:?}");
            assert!(record.entries[0].checksum_verified);
            let target = root.path().join("target/sparse");
            let allocation = allocated(&File::open(&target).unwrap()).unwrap();
            assert!(
                allocation < expected.len() as u64 / 4,
                "{allocation} bytes allocated"
            );
            assert_eq!(fs::read(target).unwrap(), expected);
            assert_eq!(source.exists(), operation == Operation::Copy);
            if source.exists() {
                assert_eq!(
                    crate::fs::inspect(&source).identity,
                    plan.operands[0].identity
                );
            }
            eprintln!(
                "NATIVE_SPARSE_EXERCISED: {dense} baseline, {allocation} allocated, {} logical",
                expected.len()
            );
        }
    }

    #[test]
    fn cancelled_ntfs_sparse_move_retains_source_and_previous_destination_identity() {
        let (root, plan, expected) = fixture();
        let target = root.path().join("target/sparse");
        let previous = crate::fs::inspect(&target).identity;
        let cancel = AtomicBool::new(false);
        let record = operations::execute(&plan, Options { checksum: true }, &cancel, |event| {
            if matches!(
                event,
                Event::Progress {
                    stage: Stage::Reading,
                    ..
                }
            ) {
                cancel.store(true, Ordering::Relaxed);
            }
            Decision::Replace
        });
        assert_eq!(record.entries[0].outcome, Outcome::Cancelled);
        assert_eq!(fs::read(&plan.operands[0].path).unwrap(), expected);
        assert_eq!(
            crate::fs::inspect(&plan.operands[0].path).identity,
            plan.operands[0].identity
        );
        assert_eq!(fs::read(&target).unwrap(), b"previous");
        assert_eq!(crate::fs::inspect(&target).identity, previous);
        assert_eq!(fs::read_dir(root.path().join("target")).unwrap().count(), 1);
    }
}
