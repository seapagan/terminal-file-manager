use super::*;
use std::sync::Mutex;

#[test]
fn listing_metrics_count_only_successful_targeted_refreshes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("known");
    std::fs::write(&path, "old").unwrap();
    let context = IgnoreContext::isolated();
    let seed = fs::list(root.path(), &context, &AtomicBool::new(false)).unwrap();
    let request = Request {
        pane: 0,
        kind: RequestKind::Listing,
        generation: 1,
        path: root.path().into(),
    };
    for (paths, cancelled, targeted, scans) in [
        (vec![path.clone()], false, 1, 0),
        (vec![path], true, 0, 0),
        (vec![root.path().join("unknown")], false, 0, 1),
    ] {
        let metrics = Metrics::default();
        let value = execute_work(
            &request,
            &context,
            &AtomicBool::new(cancelled),
            Some((seed.clone(), paths)),
            &metrics,
        );
        assert_eq!(matches!(value, Value::Listing(Err(_))), cancelled);
        assert_eq!(metrics.metadata_refreshes.load(Ordering::Relaxed), targeted);
        assert_eq!(metrics.full_scans.load(Ordering::Relaxed), scans);
    }
}

#[test]
fn panic_releases_each_lane_and_runs_only_the_latest_pending_request() {
    for kind in [
        RequestKind::Listing,
        RequestKind::Metadata,
        RequestKind::Totals,
    ] {
        recover_lane(kind);
    }
}

fn recover_lane(kind: RequestKind) {
    let root = tempfile::tempdir().unwrap();
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let (running, release) = panic_first(&mut pool, kind);
    let request = Request {
        pane: 0,
        kind,
        generation: 1,
        path: root.path().into(),
    };
    pool.submit(request.clone());
    running.recv_timeout(Duration::from_secs(10)).unwrap();
    for generation in 2..=10 {
        pool.submit(Request {
            generation,
            ..request.clone()
        });
    }
    pool.submit(Request {
        pane: 1,
        ..request.clone()
    });
    let other = pool.next(Duration::from_secs(10)).unwrap();
    assert_eq!(other.request.pane, 1);
    assert_eq!(pool.pending_count(), 1);
    release.send(()).unwrap();
    let failed = pool
        .next(Duration::from_secs(10))
        .expect("panic must complete");
    assert_eq!(failed.request.generation, 1);
    assert_eq!(failed.request.pane, 0);
    let Value::Failed(error) = failed.value else {
        panic!("worker failure must be explicit")
    };
    assert!(error.contains("controlled worker failure"));
    assert!(error.contains("worker failed"));
    let latest = pool
        .next(Duration::from_secs(10))
        .expect("lane must recover");
    assert_eq!(latest.request.generation, 10);
    assert!(!matches!(latest.value, Value::Failed(_)));
    assert_eq!(pool.active_count(), 0);
    assert_eq!(pool.pending_count(), 0);
}

fn panic_first(pool: &mut Pool, kind: RequestKind) -> (mpsc::Receiver<()>, mpsc::SyncSender<()>) {
    let (entered, running) = mpsc::sync_channel(1);
    let (release, wait) = mpsc::sync_channel(1);
    let wait = Mutex::new(wait);
    pool.before_execute = Some(Arc::new(move |request, cancelled| {
        if request.pane == 0 && request.kind == kind && request.generation == 1 {
            entered.send(()).unwrap();
            wait.lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
            assert!(cancelled.load(Ordering::Relaxed));
            panic!("controlled worker failure");
        }
    }));
    (running, release)
}
