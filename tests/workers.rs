//! Real bounded background I/O, with deterministic generation assertions.
use std::{
    fs,
    time::{Duration, Instant},
};
use terminal_file_manager::{
    app::{App, Request, RequestKind},
    fs::IgnoreContext,
    workers::{Pool, Value},
};

#[test]
fn burst_requests_coalesce_and_latest_listing_installs() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("sentinel"), "unchanged").unwrap();
    let mut app = App::new(root.path().to_path_buf());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut latest = 0;
    for _ in 0..1000 {
        let request = app.request_listing(0, root.path().to_path_buf());
        latest = request.generation;
        pool.submit(request);
        assert!(pool.pending_count() <= 6);
        assert!(pool.active_count() <= 6);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            Instant::now() < deadline,
            "latest generation did not finish"
        );
        if let Some(completion) = pool.next(Duration::from_millis(20)) {
            let generation = completion.request.generation;
            if let Value::Listing(result) = completion.value {
                app.complete_listing(0, generation, result);
                if generation == latest {
                    break;
                }
            }
        }
    }
    assert_eq!(app.panes[0].entries[0].name, "sentinel");
    assert_eq!(
        fs::read(root.path().join("sentinel")).unwrap(),
        b"unchanged"
    );
}

#[test]
fn metadata_and_counts_use_separate_bounded_lanes() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("one"), "1234").unwrap();
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    for kind in [RequestKind::Metadata, RequestKind::Totals] {
        pool.submit(Request {
            pane: 1,
            generation: 42,
            path: root.path().to_path_buf(),
            kind,
        });
    }
    let mut kinds = Vec::new();
    for _ in 0..2 {
        let completion = pool
            .next(Duration::from_secs(10))
            .expect("worker completion");
        assert_eq!(completion.request.generation, 42);
        kinds.push(completion.request.kind);
        if let Value::Totals(result) = completion.value {
            assert_eq!(result.unwrap().bytes, 4);
        }
    }
    assert!(kinds.contains(&RequestKind::Metadata));
    assert!(kinds.contains(&RequestKind::Totals));
}
