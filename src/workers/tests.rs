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
        RequestKind::ParentListing(None),
        RequestKind::Metadata,
        RequestKind::Totals,
    ] {
        recover_lane(kind);
    }
}

fn recover_lane(kind: RequestKind) {
    let root = tempfile::tempdir().unwrap();
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let slots = match kind {
        RequestKind::Listing | RequestKind::ParentListing(_) => 2,
        _ => 1,
    };
    let (running, release) = block_until_panic(&mut pool, slots);
    let request = Request {
        pane: 0,
        kind,
        generation: 1,
        path: root.path().into(),
    };
    for generation in 1..=slots {
        pool.submit(Request {
            generation,
            ..request.clone()
        });
        assert_eq!(
            running.recv_timeout(Duration::from_secs(10)).unwrap(),
            generation
        );
    }
    let newest = queue_replacement_burst(&mut pool, &request, slots);
    pool.submit(Request { pane: 1, ..request });
    let other = pool.next(Duration::from_secs(10)).unwrap();
    assert_eq!(other.request.pane, 1);
    assert_eq!(pool.pending_count(), 1);
    assert_eq!(pool.active_count() as u64, slots);
    // Release the second slot first for listings. Its completion must free that slot.
    release[slots as usize - 1].send(()).unwrap();
    assert_panic_completion(&mut pool, slots);
    let latest = pool
        .next(Duration::from_secs(10))
        .expect("lane must recover");
    assert_eq!(latest.request.generation, 10);
    assert_eq!(latest.request.kind, newest);
    assert!(!matches!(latest.value, Value::Failed(_)));
    if slots == 2 {
        assert_eq!(pool.active_count(), 1);
        release[0].send(()).unwrap();
        assert_panic_completion(&mut pool, 1);
    }
    assert_eq!(pool.active_count(), 0);
    assert_eq!(pool.pending_count(), 0);
}

fn queue_replacement_burst(pool: &mut Pool, request: &Request, slots: u64) -> RequestKind {
    for generation in slots + 1..10 {
        pool.submit(Request {
            generation,
            ..request.clone()
        });
    }
    let newest = match request.kind {
        RequestKind::Listing => RequestKind::ParentListing(None),
        RequestKind::ParentListing(_) => RequestKind::Listing,
        _ => request.kind.clone(),
    };
    pool.submit(Request {
        generation: 10,
        kind: newest.clone(),
        ..request.clone()
    });
    newest
}

fn assert_panic_completion(pool: &mut Pool, generation: u64) {
    let failed = pool
        .next(Duration::from_secs(10))
        .expect("panic must complete");
    assert_eq!(failed.request.generation, generation);
    assert_eq!(failed.request.pane, 0);
    let Value::Failed(error) = failed.value else {
        panic!("worker failure must be explicit")
    };
    assert!(error.contains("controlled worker failure"));
    assert!(error.contains("worker failed"));
}

fn block_until_panic(pool: &mut Pool, slots: u64) -> (mpsc::Receiver<u64>, Vec<mpsc::Sender<()>>) {
    let (entered, running) = mpsc::channel();
    let (release, wait): (Vec<_>, Vec<_>) = (0..slots)
        .map(|_| {
            let (sender, receiver) = mpsc::channel();
            (sender, Mutex::new(receiver))
        })
        .unzip();
    pool.before_execute = Some(Arc::new(move |request, cancelled| {
        if request.pane == 0 && request.generation <= slots {
            entered.send(request.generation).unwrap();
            let slot = (request.generation - 1) as usize;
            if wait[slot].lock().unwrap().recv().is_err() {
                return;
            }
            assert!(cancelled.load(Ordering::Relaxed));
            panic!("controlled worker failure");
        }
    }));
    (running, release)
}
