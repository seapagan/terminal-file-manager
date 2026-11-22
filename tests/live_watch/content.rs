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
    // Advance scheduler time independently of host delays, below the periodic sweep.
    // Native events and worker completion still use the real backend and bounded wait.
    let now = std::cell::Cell::new(Duration::ZERO);
    let mut clock = || now.replace(now.get() + Duration::from_millis(5));
    // READY schedules a full catch-up only after each native watch is registered.
    until_with_clock(&mut app, &mut pool, &mut watch, &mut clock, |app| {
        app.panes.iter().all(|p| p.generation > 1)
    });
    assert_eq!(pool.listing_metrics().full_scans, 2);
    assert!(app.notifications.history.is_empty());
    for size in 10..15 {
        now.set(now.get() + Duration::from_secs(1));
        let before = app.panes.each_ref().map(|pane| pane.generation);
        let metrics = pool.listing_metrics();
        std::fs::write(&file, "x".repeat(size)).unwrap();
        until_with_clock(&mut app, &mut pool, &mut watch, &mut clock, |app| {
            app.panes.iter().all(|p| {
                p.entries
                    .iter()
                    .any(|e| e.path == file && e.size == Some(size as u64))
            })
        });
        let after = pool.listing_metrics();
        let completed: u64 = app
            .panes
            .iter()
            .zip(before)
            .map(|(pane, generation)| pane.generation - generation)
            .sum();
        assert!(
            completed >= 2,
            "both panes must complete the {size}-byte update"
        );
        assert_eq!(
            after.full_scans, metrics.full_scans,
            "known-child {size}-byte update unnecessarily enumerated the directory"
        );
        assert_eq!(
            (after.metadata_refreshes - metrics.metadata_refreshes) as u64,
            completed,
            "every completed listing must use targeted metadata refresh"
        );
    }
    assert!(app.notifications.history.is_empty());
    assert_manual_listing_matches(&app, root.path());
}
