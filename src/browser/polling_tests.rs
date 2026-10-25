use super::tests::{collect_until_idle, counting_controller, hold_first_count};
use super::*;

fn reconcile(controller: &mut Controller, pool: &mut Pool) {
    let request = controller.app.request_refresh(0).unwrap();
    pool.submit(request);
    finish_listing(controller, pool);
}

fn finish_listing(controller: &mut Controller, pool: &mut Pool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while controller.app.panes[0].listing {
        collect(controller, pool);
        assert!(Instant::now() < deadline, "listing timed out");
        std::thread::yield_now();
    }
    schedule_metadata(controller, pool, &mut [None, None]);
}

#[cfg(not(windows))]
fn sweep_reconcile(controller: &mut Controller, pool: &mut Pool) {
    let mut runtime = crate::watch::Runtime::default();
    runtime.poll(&mut controller.app, pool, Duration::ZERO);
    runtime.poll(&mut controller.app, pool, Duration::from_secs(30));
    assert!(controller.app.panes[0].listing);
    finish_listing(controller, pool);
}

#[cfg(not(windows))]
fn await_native_listing(
    controller: &mut Controller,
    pool: &mut Pool,
    runtime: &mut crate::watch::Runtime,
    generation: u64,
    clock: Instant,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            Instant::now() < deadline,
            "native event reconciliation timed out"
        );
        // Freeze the sweep clock below 30 seconds: only READY or a native hint can start work.
        runtime.poll(
            &mut controller.app,
            pool,
            clock.elapsed().min(Duration::from_secs(20)),
        );
        collect(controller, pool);
        if controller.app.panes[0].generation > generation && !controller.app.panes[0].listing {
            schedule_metadata(controller, pool, &mut [None, None]);
            return;
        }
        std::thread::yield_now();
    }
}

#[test]
#[cfg(not(windows))]
fn native_event_reconciliation_preserves_running_and_completed_counts() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = counting_controller(root.path());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut runtime = crate::watch::Runtime::default();
    let clock = Instant::now();
    pool.submit(controller.app.request_metadata(0).unwrap());
    collect_until_idle(&mut controller, &mut pool);
    let initial = controller.app.panes[0].generation;
    await_native_listing(&mut controller, &mut pool, &mut runtime, initial, clock);
    let (running, release, stopped) = hold_first_count(&mut pool, false);
    let count = controller.app.request_totals().unwrap();
    pool.submit(count.clone());
    running.recv_timeout(Duration::from_secs(10)).unwrap();
    let generation = controller.app.panes[0].generation;
    controller.app.metadata_scroll = 7;
    std::fs::write(root.path().join("unrelated-to-selected-count"), "event").unwrap();
    await_native_listing(&mut controller, &mut pool, &mut runtime, generation, clock);
    assert!(controller.app.details[0].counting);
    assert_eq!(controller.app.metadata_scroll, 7);
    release.send(()).unwrap();
    stopped.recv_timeout(Duration::from_secs(10)).unwrap();
    collect_until_idle(&mut controller, &mut pool);
    let generation = controller.app.panes[0].generation;
    std::fs::write(root.path().join("another-native-event"), "event").unwrap();
    await_native_listing(&mut controller, &mut pool, &mut runtime, generation, clock);
    assert_eq!(
        controller.app.details[0].totals_generation,
        count.generation
    );
    assert_eq!(
        controller.app.details[0]
            .totals
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .bytes,
        4
    );
    assert_eq!(controller.app.metadata_scroll, 7);
    assert!(
        controller.app.operations.notices.is_empty(),
        "native backend must remain healthy"
    );
}

#[test]
#[cfg(not(windows))]
fn unchanged_automatic_reconciliation_preserves_running_counts() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = counting_controller(root.path());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    pool.submit(controller.app.request_metadata(0).unwrap());
    collect_until_idle(&mut controller, &mut pool);
    let (running, release, stopped) = hold_first_count(&mut pool, false);
    let count = controller.app.request_totals().unwrap();
    pool.submit(count.clone());
    running.recv_timeout(Duration::from_secs(10)).unwrap();
    sweep_reconcile(&mut controller, &mut pool);
    let counting = controller.app.details[0].counting;
    release.send(()).unwrap();
    stopped.recv_timeout(Duration::from_secs(10)).unwrap();
    collect_until_idle(&mut controller, &mut pool);
    assert!(counting);
    assert_eq!(
        controller.app.details[0].totals_generation,
        count.generation
    );
    assert_eq!(
        controller.app.details[0]
            .totals
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .bytes,
        4
    );
}

#[test]
#[cfg(not(windows))]
fn unchanged_automatic_reconciliation_preserves_completed_totals() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = counting_controller(root.path());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    pool.submit(controller.app.request_metadata(0).unwrap());
    collect_until_idle(&mut controller, &mut pool);
    let count = controller.app.request_totals().unwrap();
    pool.submit(count.clone());
    collect_until_idle(&mut controller, &mut pool);
    for _ in 0..2 {
        sweep_reconcile(&mut controller, &mut pool);
        assert_eq!(
            controller.app.details[0].totals_generation,
            count.generation
        );
        assert_eq!(
            controller.app.details[0]
                .totals
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap()
                .bytes,
            4
        );
    }
}

#[test]
#[cfg(not(windows))]
fn automatic_changes_cancel_running_counts_and_reject_late_results() {
    for change in ["contents", "replacement", "navigation", "error"] {
        let root = tempfile::tempdir().unwrap();
        let mut controller = counting_controller(root.path());
        let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
        let metadata = controller.app.request_metadata(0).unwrap();
        let entry = controller.app.details[0].entry.clone().unwrap();
        pool.submit(metadata.clone());
        collect_until_idle(&mut controller, &mut pool);
        let (running, release, stopped) = hold_first_count(&mut pool, true);
        let count = controller.app.request_totals().unwrap();
        pool.submit(count.clone());
        running.recv_timeout(Duration::from_secs(10)).unwrap();
        let selected = root.path().join("directory");
        match change {
            "contents" => std::fs::write(selected.join("new"), "new").unwrap(),
            "replacement" => {
                std::fs::rename(&selected, root.path().join("preserved")).unwrap();
                std::fs::create_dir(&selected).unwrap();
            }
            "navigation" => {
                pool.submit(controller.app.request_listing(0, selected));
                finish_listing(&mut controller, &mut pool);
            }
            _ => std::fs::remove_dir_all(root.path()).unwrap(),
        }
        if change != "navigation" {
            sweep_reconcile(&mut controller, &mut pool);
        }
        release.send(()).unwrap();
        stopped.recv_timeout(Duration::from_secs(10)).unwrap();
        collect_until_idle(&mut controller, &mut pool);
        assert!(!controller.app.details[0].counting, "{change}");
        assert!(controller.app.details[0].totals.is_none(), "{change}");
        assert!(
            !controller.app.complete_metadata(&metadata, entry, None),
            "{change}"
        );
        assert!(
            !controller
                .app
                .complete_totals(&count, Ok(Default::default())),
            "{change}"
        );
    }
}

#[test]
fn automatic_metadata_changes_invalidate_completed_counts() {
    for change in [
        "size",
        "modified",
        "mode",
        "identity",
        "entry error",
        "partial listing",
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut controller = counting_controller(root.path());
        controller.app.request_metadata(0).unwrap();
        let count = controller.app.request_totals().unwrap();
        assert!(
            controller
                .app
                .complete_totals(&count, Ok(Default::default()))
        );
        let mut snapshot = crate::fs::list(
            root.path(),
            &IgnoreContext::isolated(),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        let entry = &mut snapshot.entries[0];
        match change {
            "size" => entry.size = Some(u64::MAX),
            "modified" => entry.modified = Some(std::time::SystemTime::UNIX_EPOCH),
            "mode" => entry.mode = Some(0),
            "identity" => entry.identity = None,
            "entry error" => entry.error = Some("unavailable".into()),
            _ => snapshot.errors.push("partial listing".into()),
        }
        let request = controller.app.request_reconciliation(0).unwrap();
        assert!(
            controller
                .app
                .complete_listing(0, request.generation, Ok(snapshot))
        );
        assert!(controller.app.details[0].totals.is_none(), "{change}");
        assert!(
            !controller
                .app
                .complete_totals(&count, Ok(Default::default())),
            "{change}"
        );
    }
}

#[test]
fn automatic_access_time_change_preserves_count_and_updates_cached_metadata() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = counting_controller(root.path());
    let metadata = controller.app.request_metadata(0).unwrap();
    let count = controller.app.request_totals().unwrap();
    let mut snapshot = crate::fs::list(
        root.path(),
        &IgnoreContext::isolated(),
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    snapshot.entries[0].accessed = Some(std::time::SystemTime::UNIX_EPOCH);
    let request = controller.app.request_reconciliation(0).unwrap();
    assert!(
        controller
            .app
            .complete_listing(0, request.generation, Ok(snapshot))
    );
    assert!(controller.app.details[0].counting);
    assert_eq!(controller.app.details[0].generation, metadata.generation);
    assert_eq!(
        controller.app.details[0].entry.as_ref().unwrap().accessed,
        Some(std::time::SystemTime::UNIX_EPOCH)
    );
    assert!(
        controller
            .app
            .complete_totals(&count, Ok(Default::default()))
    );
}

#[test]
fn selected_detail_change_resets_metadata_viewports_without_cursor_motion() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = counting_controller(root.path());
    controller.app.request_metadata(0).unwrap();
    let selected = controller.app.panes[0].cursor.clone();
    controller.app.popup = true;
    controller.app.metadata_scroll = 8;
    controller.app.overlay_scroll = 9;
    std::fs::write(root.path().join("directory/new-child"), "changed").unwrap();
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    pool.submit(controller.app.request_reconciliation(0).unwrap());
    finish_listing(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].cursor, selected);
    assert_eq!(controller.app.metadata_scroll, 0);
    assert_eq!(controller.app.overlay_scroll, 0);
}

#[test]
fn explicit_refresh_invalidates_running_and_completed_directory_counts() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = counting_controller(root.path());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    pool.submit(controller.app.request_metadata(0).unwrap());
    collect_until_idle(&mut controller, &mut pool);
    let (running, release, stopped) = hold_first_count(&mut pool, true);
    let count = controller.app.request_totals().unwrap();
    pool.submit(count.clone());
    running.recv_timeout(Duration::from_secs(10)).unwrap();
    reconcile(&mut controller, &mut pool);
    assert!(!controller.app.details[0].counting);
    assert_ne!(
        controller.app.details[0].totals_generation,
        count.generation
    );
    release.send(()).unwrap();
    stopped.recv_timeout(Duration::from_secs(10)).unwrap();
    collect_until_idle(&mut controller, &mut pool);
    assert!(
        !controller
            .app
            .complete_totals(&count, Ok(Default::default()))
    );
    let fresh = controller.app.request_totals().unwrap();
    pool.submit(fresh);
    collect_until_idle(&mut controller, &mut pool);
    assert_eq!(
        controller.app.details[0]
            .totals
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .bytes,
        4
    );
    reconcile(&mut controller, &mut pool);
    assert!(controller.app.details[0].totals.is_none());
}

#[test]
fn explicit_directory_replacement_invalidates_old_count_and_metadata_results() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = counting_controller(root.path());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let metadata = controller.app.request_metadata(0).unwrap();
    let entry = controller.app.details[0].entry.clone().unwrap();
    let count = controller.app.request_totals().unwrap();
    std::fs::rename(root.path().join("directory"), root.path().join("preserved")).unwrap();
    std::fs::create_dir(root.path().join("directory")).unwrap();
    reconcile(&mut controller, &mut pool);
    assert!(!controller.app.details[0].counting);
    assert!(!controller.app.complete_metadata(&metadata, entry, None));
    assert!(
        !controller
            .app
            .complete_totals(&count, Ok(Default::default()))
    );
    assert_eq!(
        std::fs::read(root.path().join("preserved/file")).unwrap(),
        b"1234"
    );
}
