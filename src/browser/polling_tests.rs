use super::tests::{collect_until_idle, counting_controller, hold_first_count};
use super::*;

fn reconcile(controller: &mut Controller, pool: &mut Pool) {
    let request = controller.app.request_reconciliation(0).unwrap();
    pool.submit(request);
    let deadline = Instant::now() + Duration::from_secs(10);
    while controller.app.panes[0].listing {
        collect(controller, pool);
        assert!(Instant::now() < deadline, "periodic listing timed out");
        std::thread::yield_now();
    }
    schedule_metadata(controller, pool, &mut [None, None]);
}

#[test]
fn unchanged_periodic_listings_preserve_running_and_completed_directory_counts() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = counting_controller(root.path());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    pool.submit(controller.app.request_metadata(0).unwrap());
    collect_until_idle(&mut controller, &mut pool);
    let (running, release, stopped) = hold_first_count(&mut pool, false);
    let count = controller.app.request_totals().unwrap();
    pool.submit(count.clone());
    running.recv_timeout(Duration::from_secs(10)).unwrap();
    reconcile(&mut controller, &mut pool);
    assert!(controller.app.details[0].counting);
    assert_eq!(
        controller.app.details[0].totals_generation,
        count.generation
    );
    release.send(()).unwrap();
    stopped.recv_timeout(Duration::from_secs(10)).unwrap();
    collect_until_idle(&mut controller, &mut pool);
    reconcile(&mut controller, &mut pool);
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
    assert_eq!(
        controller.app.details[0].totals_generation,
        count.generation
    );
    let explicit = controller.app.request_refresh(0).unwrap();
    pool.submit(explicit);
    collect_until_idle(&mut controller, &mut pool);
    assert!(controller.app.details[0].totals.is_none());
    assert!(
        !controller
            .app
            .complete_totals(&count, Ok(Default::default()))
    );
}

#[test]
fn periodic_directory_replacement_invalidates_old_count_and_metadata_results() {
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
