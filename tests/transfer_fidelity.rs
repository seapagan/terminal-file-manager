use std::{
    fs,
    time::{Duration, SystemTime},
};
use terminal_file_manager::operations::{Decision, Operation, Options, Plan, execute};

#[test]
fn destination_access_time_uses_metadata_frozen_before_reads_and_checksum() {
    for operation in [Operation::Copy, Operation::Move] {
        for checksum in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            fs::create_dir(root.path().join("target")).unwrap();
            fs::write(&source, "verified bytes").unwrap();
            fs::File::options()
                .write(true)
                .open(&source)
                .unwrap()
                .set_times(
                    fs::FileTimes::new()
                        .set_accessed(SystemTime::UNIX_EPOCH + Duration::from_secs(946_684_800))
                        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(4_102_444_800)),
                )
                .unwrap();
            let before = fs::metadata(&source).unwrap();
            let plan = Plan::paths(
                operation,
                vec![source.clone()],
                Some(root.path().join("target")),
            )
            .unwrap();
            let record = execute(&plan, Options { checksum }, &false.into(), |_| {
                Decision::Skip
            });
            assert_eq!(record.completed_files(), 1, "{record:?}");
            let target = root.path().join("target/source");
            let after = fs::metadata(&target).unwrap();
            assert_eq!(
                after
                    .accessed()
                    .unwrap()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                before
                    .accessed()
                    .unwrap()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            );
            assert_eq!(after.modified().unwrap(), before.modified().unwrap());
            assert_eq!(fs::read(target).unwrap(), b"verified bytes");
            assert_eq!(source.exists(), operation == Operation::Copy);
        }
    }
}

#[cfg(unix)]
#[test]
fn large_sparse_copy_and_move_keep_holes_and_verify_all_logical_bytes() {
    use std::{
        io::{Seek, SeekFrom, Write},
        os::unix::fs::MetadataExt,
    };
    for operation in [Operation::Copy, Operation::Move] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("sparse");
        fs::create_dir(root.path().join("target")).unwrap();
        let mut file = fs::File::create(&source).unwrap();
        file.write_all(b"head").unwrap();
        file.seek(SeekFrom::Start(64 * 1024 * 1024 - 4)).unwrap();
        file.write_all(b"tail").unwrap();
        file.sync_all().unwrap();
        let before = file.metadata().unwrap();
        require_sparse_fixture(&source, &before);
        drop(file);
        let expected = fs::read(&source).unwrap();
        let plan = Plan::paths(
            operation,
            vec![source.clone()],
            Some(root.path().join("target")),
        )
        .unwrap();
        let record = execute(&plan, Options { checksum: true }, &false.into(), |_| {
            Decision::Skip
        });
        assert_eq!(record.completed_files(), 1, "{record:?}");
        assert!(record.entries[0].checksum_verified);
        let target = root.path().join("target/sparse");
        let after = fs::metadata(&target).unwrap();
        assert_eq!(after.len(), before.len());
        assert!(
            after.blocks() * 512 < after.len() / 4,
            "sparse payload expanded: {} allocated of {} logical bytes",
            after.blocks() * 512,
            after.len()
        );
        assert_eq!(fs::read(target).unwrap(), expected);
        assert_eq!(source.exists(), operation == Operation::Copy);
    }
}

#[cfg(unix)]
fn require_sparse_fixture(source: &std::path::Path, before: &fs::Metadata) {
    use std::os::unix::fs::MetadataExt;
    #[cfg(target_os = "macos")]
    if before.blocks() * 512 >= before.len() / 4 {
        eprintln!(
            "Sparse fixture filesystem: {:?}",
            std::process::Command::new("/sbin/mount").output().unwrap()
        );
    }
    assert!(
        before.blocks() * 512 < before.len() / 4,
        "fixture {} must support sparse files: {} allocated of {} logical bytes",
        source.display(),
        before.blocks() * 512,
        before.len()
    );
}

#[cfg(unix)]
#[test]
fn interleaved_page_sized_holes_and_a_trailing_hole_survive_copy() {
    use std::{
        io::{Seek, SeekFrom, Write},
        os::unix::fs::MetadataExt,
    };
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("pages");
    let mut file = fs::File::create(&source).unwrap();
    file.set_len(1024 * 1024).unwrap();
    for page in 0..128 {
        file.seek(SeekFrom::Start(page * 8192)).unwrap();
        file.write_all(&vec![b'x'; 4096]).unwrap();
    }
    file.sync_all().unwrap();
    let before = file.metadata().unwrap();
    drop(file);
    let expected = fs::read(&source).unwrap();
    fs::create_dir(root.path().join("target")).unwrap();
    let plan = Plan::paths(
        Operation::Copy,
        vec![source],
        Some(root.path().join("target")),
    )
    .unwrap();
    let record = execute(&plan, Options { checksum: true }, &false.into(), |_| {
        Decision::Skip
    });
    assert_eq!(record.completed_files(), 1, "{record:?}");
    let target = root.path().join("target/pages");
    let after = fs::metadata(&target).unwrap();
    assert!(
        after.blocks() * 512 <= before.blocks() * 512 + 64 * 1024,
        "{} source / {} destination allocated",
        before.blocks() * 512,
        after.blocks() * 512
    );
    assert_eq!(fs::read(target).unwrap(), expected);
}
