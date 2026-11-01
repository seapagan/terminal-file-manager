//! Native setup notifications may outlive registration; noise must not change the view.
use super::*;

#[test]
fn native_reads_and_parent_chatter_keep_reconciliation_bounded() {
    for _ in 0..10 {
        chatter_preserves_view();
    }
}

fn chatter_preserves_view() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("current");
    std::fs::create_dir(&path).unwrap();
    let selected = path.join("selected");
    std::fs::write(&selected, "data").unwrap();
    let mut app = loaded(&path);
    app.panes[0].toggle_mark();
    let marks = app.panes[0].marks.clone();
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    let mut starts = until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| p.generation > 1)
    });
    let observed = observe_chatter(&mut app, &mut pool, &mut watch, started, root.path());
    for pane in 0..2 {
        // FSEvents may deliver fixture creation after the registration catch-up.
        starts[pane].extend(&observed[pane]);
        assert!(
            starts[pane]
                .windows(2)
                .all(|pair| pair[1] - pair[0] >= Duration::from_secs(1))
        );
        let state = &app.panes[pane];
        assert!(state.error.is_none());
        assert_eq!(state.path, path);
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.current().unwrap().name, "selected");
        assert_eq!(state.cursor.as_deref(), Some(selected.as_path()));
    }
    assert_eq!(app.panes[0].marks, marks);
    assert!(app.panes[1].marks.is_empty());
    assert_eq!(std::fs::read(selected).unwrap(), b"data");
}

fn observe_chatter(
    app: &mut App,
    pool: &mut Pool,
    watch: &mut Runtime,
    started: Instant,
    parent: &Path,
) -> [Vec<Duration>; 2] {
    let path = app.panes[0].path.clone();
    let mut generations = [app.panes[0].generation, app.panes[1].generation];
    let mut starts: [Vec<Duration>; 2] = Default::default();
    for index in 0..100 {
        std::fs::read(path.join("selected")).unwrap();
        fs::list(&path, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap();
        std::fs::write(parent.join(format!("sibling-{index}")), "unrelated").unwrap();
        let now = started.elapsed();
        watch.poll(app, pool, now);
        for pane in 0..2 {
            if app.panes[pane].generation != generations[pane] {
                starts[pane].push(now);
                generations[pane] = app.panes[pane].generation;
            }
        }
        assert!(pool.active_count() <= 2 && pool.pending_count() <= 2);
        if let Some(completion) = pool.next(Duration::from_millis(10)) {
            let Value::Listing(result) = completion.value else {
                panic!("unexpected non-listing completion");
            };
            assert!(result.is_ok(), "native listing failed: {result:?}");
            app.complete_listing(
                completion.request.pane,
                completion.request.generation,
                result,
            );
        }
    }
    let tail = until(app, pool, watch, started, |_| true);
    for pane in 0..2 {
        starts[pane].extend(&tail[pane]);
    }
    starts
}
