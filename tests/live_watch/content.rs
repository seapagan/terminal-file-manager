use super::*;

#[cfg(target_os = "linux")]
#[test]
fn known_child_writes_update_metadata_without_repeated_directory_scans() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("ready");
    std::fs::write(&file, "initial").unwrap();
    let mut app = loaded(root.path());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    std::fs::write(root.path().join("registered"), "owned").unwrap();
    converge(&mut app, &mut pool, &mut watch, started, "registered");
    let before = app.panes[0].generation;
    let metrics = pool.listing_metrics();
    for size in 10..15 {
        std::fs::write(&file, "x".repeat(size)).unwrap();
        until(&mut app, &mut pool, &mut watch, started, |app| {
            app.panes.iter().all(|p| {
                p.entries
                    .iter()
                    .any(|e| e.path == file && e.size == Some(size as u64))
            })
        });
    }
    eprintln!(
        "Five same-child writes: {} listing requests/pane",
        app.panes[0].generation - before
    );
    let after = pool.listing_metrics();
    eprintln!(
        "N5 executed work: full scans {}, metadata refreshes {}",
        after.full_scans - metrics.full_scans,
        after.metadata_refreshes - metrics.metadata_refreshes
    );
    assert!(after.metadata_refreshes > metrics.metadata_refreshes);
    assert!(after.full_scans - metrics.full_scans < 10);
    assert_manual_listing_matches(&app, root.path());
}
