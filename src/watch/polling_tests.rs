use super::tests::{finish, prepared};
use super::*;
use crate::fs::{self, IgnoreContext};
use std::sync::atomic::AtomicBool;

fn polling() -> (tempfile::TempDir, App, Pool, Runtime) {
    let (root, mut app, mut pool, _) = prepared();
    let mut runtime = Runtime::new(Mode::Polling);
    runtime.poll(&mut app, &mut pool, Duration::ZERO);
    (root, app, pool, runtime)
}

#[test]
#[cfg(windows)]
fn windows_default_rebinds_without_native_owners_or_failure_notices() {
    let (root, mut app, mut pool, _) = prepared();
    let mut runtime = Runtime::default();
    runtime.poll(&mut app, &mut pool, Duration::ZERO);
    assert!(runtime.slots.iter().all(|slot| slot.worker.is_none()));
    std::fs::write(root.path().join("periodic"), "data").unwrap();
    runtime.poll(&mut app, &mut pool, Duration::from_secs(2));
    finish(&mut app, &mut pool);
    assert!(
        app.panes
            .iter()
            .all(|pane| pane.current().unwrap().name == "periodic")
    );
    assert!(app.operations.notices.is_empty());
}

#[test]
fn intentional_polling_updates_both_panes_every_two_seconds_without_native_warnings() {
    let (root, mut app, mut pool, mut runtime) = polling();
    assert!(!app.dual);
    std::fs::write(root.path().join("first"), "data").unwrap();
    runtime.poll(&mut app, &mut pool, Duration::from_millis(1999));
    assert_eq!(pool.active_count(), 0);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(2));
    assert_eq!(pool.active_count(), 2);
    finish(&mut app, &mut pool);
    assert!(
        app.panes
            .iter()
            .all(|pane| pane.current().unwrap().name == "first")
    );
    std::fs::write(root.path().join("second"), "data").unwrap();
    runtime.poll(&mut app, &mut pool, Duration::from_millis(3999));
    assert_eq!(pool.active_count(), 0);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(4));
    finish(&mut app, &mut pool);
    assert!(app.panes.iter().all(|pane| pane.entries.len() == 2));
    assert!(app.operations.notices.is_empty());
    assert!(
        runtime
            .slots
            .iter()
            .all(|slot| slot.worker.is_none() && !slot.failed)
    );
}

#[test]
fn polling_deadlines_preserve_an_unchanged_directory_count_target() {
    let (root, mut app, mut pool, mut runtime) = polling();
    std::fs::create_dir(root.path().join("counted")).unwrap();
    pool.submit(app.request_refresh(0).unwrap());
    finish(&mut app, &mut pool);
    app.request_metadata(0).unwrap();
    let count = app.request_totals().unwrap();
    runtime.poll(&mut app, &mut pool, Duration::from_secs(2));
    finish(&mut app, &mut pool);
    assert!(app.details[0].counting);
    assert_eq!(app.details[0].totals_generation, count.generation);
    assert!(app.complete_totals(&count, Ok(Default::default())));
    runtime.poll(&mut app, &mut pool, Duration::from_secs(4));
    finish(&mut app, &mut pool);
    assert!(app.details[0].totals.is_some());
    assert_eq!(app.details[0].totals_generation, count.generation);
}

#[test]
fn overdue_polls_coalesce_behind_in_flight_listings_without_cancellation() {
    let (_root, mut app, mut pool, mut runtime) = polling();
    runtime.poll(&mut app, &mut pool, Duration::from_secs(2));
    let generations = [app.panes[0].generation, app.panes[1].generation];
    for time in 2001..8000 {
        runtime.poll(&mut app, &mut pool, Duration::from_millis(time));
    }
    assert_eq!(
        [app.panes[0].generation, app.panes[1].generation],
        generations
    );
    assert_eq!(pool.active_count(), 2);
    assert_eq!(pool.pending_count(), 0);
    finish(&mut app, &mut pool);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(8));
    assert_eq!(app.panes[0].generation, generations[0] + 1);
    assert_eq!(app.panes[1].generation, generations[1] + 1);
    finish(&mut app, &mut pool);
    runtime.poll(&mut app, &mut pool, Duration::from_millis(9999));
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn polling_deadline_cannot_hijack_navigation_or_accept_its_stale_completion() {
    let (_root, mut app, mut pool, mut runtime) = polling();
    let old = app.request_refresh(0).unwrap();
    let stale = fs::list(
        &old.path,
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let next = tempfile::tempdir().unwrap();
    std::fs::write(next.path().join("new-context"), "data").unwrap();
    let navigation = app.request_listing(0, next.path().into());
    runtime.poll(&mut app, &mut pool, Duration::from_secs(2));
    assert_eq!(app.panes[0].generation, navigation.generation);
    assert!(!app.complete_listing(0, old.generation, Ok(stale)));
    let snapshot = fs::list(
        next.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    app.complete_listing(0, navigation.generation, Ok(snapshot));
    runtime.poll(&mut app, &mut pool, Duration::from_secs(3));
    finish(&mut app, &mut pool);
    assert_eq!(app.panes[0].path, next.path());
    assert_eq!(app.panes[0].current().unwrap().name, "new-context");
    let generation = app.panes[0].generation;
    runtime.poll(&mut app, &mut pool, Duration::from_millis(4999));
    assert_eq!(app.panes[0].generation, generation);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(5));
    assert_eq!(app.panes[0].generation, generation + 1);
    finish(&mut app, &mut pool);
}
