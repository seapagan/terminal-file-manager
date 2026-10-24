use std::{
    fs,
    io::Write,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime},
};
use terminal_file_manager::operations::{
    Decision, Event, Operation, Options, Outcome, Plan, Stage, execute,
};

#[test]
fn newer_only_replaces_strictly_older_destinations_with_native_timestamps() {
    for source_newer in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(&source, "incoming").unwrap();
        fs::write(target.join("source"), "previous").unwrap();
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        fs::OpenOptions::new()
            .write(true)
            .open(&source)
            .unwrap()
            .set_modified(base + Duration::from_secs(if source_newer { 3600 } else { 0 }))
            .unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(target.join("source"))
            .unwrap()
            .set_modified(base)
            .unwrap();
        let plan =
            Plan::paths(Operation::Copy, vec![source.clone()], Some(target.clone())).unwrap();
        let record = execute(&plan, Options::default(), &false.into(), |_| {
            Decision::Newer
        });
        assert_eq!(
            record.entries[0].outcome,
            if source_newer {
                Outcome::Completed
            } else {
                Outcome::Skipped
            }
        );
        assert_eq!(
            fs::read(target.join("source")).unwrap(),
            if source_newer {
                b"incoming"
            } else {
                b"previous"
            }
        );
        assert_eq!(fs::read(source).unwrap(), b"incoming");
    }
}

#[test]
fn recursive_move_removes_only_completed_source_directories_after_merge() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::create_dir_all(target.join("source/nested")).unwrap();
    fs::write(source.join("nested/moved"), "incoming").unwrap();
    fs::write(target.join("source/nested/unrelated"), "preserved").unwrap();
    let plan = Plan::paths(Operation::Move, vec![source.clone()], Some(target.clone())).unwrap();
    let record = execute(&plan, Options::default(), &false.into(), |_| Decision::Skip);
    assert!(
        record
            .entries
            .iter()
            .all(|entry| entry.outcome == Outcome::Completed)
    );
    assert!(!source.exists());
    assert_eq!(record.completed_files(), 1);
    assert_eq!(
        fs::read(target.join("source/nested/moved")).unwrap(),
        b"incoming"
    );
    assert_eq!(
        fs::read(target.join("source/nested/unrelated")).unwrap(),
        b"preserved"
    );
}

#[test]
fn cancellation_after_commit_keeps_the_verified_destination_and_move_source() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(&source, "verified").unwrap();
    let plan = Plan::paths(Operation::Move, vec![source.clone()], Some(target.clone())).unwrap();
    let cancel = AtomicBool::new(false);
    let record = execute(&plan, Options::default(), &cancel, |event| {
        if matches!(
            event,
            Event::Progress {
                stage: Stage::Committed,
                ..
            }
        ) {
            cancel.store(true, Ordering::Relaxed);
        }
        Decision::Skip
    });
    assert!(cancel.load(Ordering::Relaxed));
    assert_eq!(record.entries[0].outcome, Outcome::CopiedSourceRetained);
    assert_eq!(record.completed_files(), 0);
    assert_eq!(fs::read(source).unwrap(), b"verified");
    assert_eq!(fs::read(target.join("source")).unwrap(), b"verified");
}

#[test]
fn transfer_verification_failures_preserve_move_source_and_previous_destination() {
    for (fault, expected_detail, source_bytes) in [
        ("size", "Staged destination size mismatch", "original"),
        ("checksum", "SHA-256 verification failed", "original"),
        ("source", "Source", "changed source"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir(&target).unwrap();
        future_dated_source(&source);
        fs::write(target.join("source"), "previous").unwrap();
        let plan =
            Plan::paths(Operation::Move, vec![source.clone()], Some(target.clone())).unwrap();
        let mut payload = None;
        let mut finalized = false;
        let record = execute(&plan, Options { checksum: true }, &false.into(), |event| {
            match event {
                Event::OwnedStage { payload: entry, .. } => payload = Some(entry.path),
                Event::Progress {
                    stage: Stage::Reading,
                    ..
                } => {
                    corrupt_transfer(fault, payload.as_ref().unwrap(), &source);
                }
                Event::Progress {
                    stage: Stage::Verified | Stage::Committed | Stage::Removing,
                    ..
                } => finalized = true,
                _ => {}
            }
            Decision::Replace
        });
        assert!(!finalized, "{fault}: {record:?}");
        assert_eq!(record.completed_files(), 0);
        let entry = &record.entries[0];
        assert_eq!(entry.outcome, Outcome::Changed, "{fault}: {record:?}");
        assert!(
            entry.detail.contains(expected_detail),
            "{fault}: {record:?}"
        );
        assert!(!entry.checksum_verified);
        assert!(entry.recovery.is_none());
        assert_eq!(fs::read(&source).unwrap(), source_bytes.as_bytes());
        assert_eq!(fs::read(target.join("source")).unwrap(), b"previous");
        assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
        assert!(!payload.unwrap().parent().unwrap().exists());
    }
}

fn corrupt_transfer(fault: &str, payload: &std::path::Path, source: &std::path::Path) {
    match fault {
        "size" => fs::OpenOptions::new()
            .write(true)
            .open(payload)
            .unwrap()
            .set_len(0)
            .unwrap(),
        "checksum" => fs::write(payload, "tampered").unwrap(),
        "source" => fs::write(source, "changed source").unwrap(),
        _ => unreachable!(),
    }
}

fn future_dated_source(source: &std::path::Path) {
    let mut file = fs::File::create(source).unwrap();
    file.write_all(b"original").unwrap();
    // Keep APFS birth time stable even when verification fails before stage sealing.
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000))
        .unwrap();
}
