use super::*;
use crate::{
    fs::{self, IgnoreContext},
    workers::Value,
};
use std::sync::atomic::AtomicBool;

fn prepared() -> (tempfile::TempDir, App, Pool, Runtime) {
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(root.path().into());
    let mut runtime = Runtime::default();
    for pane in 0..2 {
        app.complete_listing(
            pane,
            1,
            Ok(fs::list(
                root.path(),
                &IgnoreContext::isolated(),
                &AtomicBool::new(false),
            )
            .unwrap()),
        );
        let slot = &mut runtime.slots[pane];
        slot.worker = None;
        slot.path = Some(app.panes[pane].path.clone());
        slot.context = app.panes[pane].context;
    }
    (
        root,
        app,
        Pool::new(IgnoreContext::isolated()).unwrap(),
        runtime,
    )
}

fn finish(app: &mut App, pool: &mut Pool) {
    while pool.active_count() != 0 {
        let completion = pool
            .next(Duration::from_secs(5))
            .expect("listing completion");
        let Value::Listing(result) = completion.value else {
            panic!("listing lane")
        };
        app.complete_listing(
            completion.request.pane,
            completion.request.generation,
            result,
        );
    }
}

#[test]
fn healthy_silent_watcher_reconciles_unconditionally_at_thirty_seconds() {
    let (root, mut app, mut pool, mut runtime) = prepared();
    std::fs::write(root.path().join("silent-change"), "data").unwrap();
    runtime.poll(&mut app, &mut pool, Duration::from_millis(29999));
    assert_eq!(pool.active_count(), 0);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(30));
    assert_eq!(pool.active_count(), 2);
    finish(&mut app, &mut pool);
    assert!(
        app.panes
            .iter()
            .all(|p| p.current().unwrap().name == "silent-change")
    );
    runtime.poll(&mut app, &mut pool, Duration::from_secs(31));
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn dirty_during_scan_never_cancels_it_and_requires_a_final_follow_up() {
    let (_root, mut app, mut pool, mut runtime) = prepared();
    pool.submit(app.request_refresh(0).unwrap());
    let generation = app.panes[0].generation;
    for time in 0..5000 {
        runtime.slots[0]
            .signals
            .bits
            .fetch_or(DIRTY, Ordering::Release);
        runtime.poll(&mut app, &mut pool, Duration::from_millis(time));
    }
    assert_eq!(app.panes[0].generation, generation);
    assert_eq!(pool.pending_count(), 0);
    finish(&mut app, &mut pool);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(5));
    assert_eq!(app.panes[0].generation, generation + 1);
    finish(&mut app, &mut pool);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(6));
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn repeated_errors_are_bounded_and_fallback_recovers_without_warning_spam() {
    let (root, mut app, mut pool, mut runtime) = prepared();
    for time in (0..=2000).step_by(10) {
        runtime.slots[0]
            .signals
            .bits
            .store(ERROR | DIRTY, Ordering::Release);
        runtime.poll(&mut app, &mut pool, Duration::from_millis(time));
        finish(&mut app, &mut pool);
    }
    assert!(app.panes[0].generation <= 4);
    assert_eq!(app.operations.notices.len(), 1);
    assert!(runtime.slots[0].failed);
    std::fs::write(root.path().join("fallback"), "data").unwrap();
    runtime.poll(&mut app, &mut pool, Duration::from_secs(5));
    finish(&mut app, &mut pool);
    assert_eq!(app.panes[0].current().unwrap().name, "fallback");
    runtime.slots[0]
        .signals
        .bits
        .store(READY, Ordering::Release);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(6));
    assert!(!runtime.slots[0].failed);
    assert_eq!(app.operations.notices.len(), 2);
}

#[test]
fn old_subscription_cannot_dirty_a_new_context_or_override_pending_navigation() {
    let (_root, mut app, mut pool, mut runtime) = prepared();
    let next = tempfile::tempdir().unwrap();
    let old = runtime.slots[0].signals.clone();
    let navigation = app.request_listing(0, next.path().into());
    old.bits.store(DIRTY, Ordering::Release);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(30));
    assert_eq!(app.panes[0].generation, navigation.generation);
    assert_eq!(app.panes[0].pending.as_deref(), Some(next.path()));
    app.complete_listing(
        0,
        navigation.generation,
        Ok(fs::list(
            next.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    runtime.poll(&mut app, &mut pool, Duration::from_secs(31));
    finish(&mut app, &mut pool);
    old.bits.store(DIRTY | ERROR, Ordering::Release);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(32));
    assert_eq!(pool.active_count(), 0);
    assert_eq!(app.panes[0].path, next.path());
}

#[test]
fn initial_listing_error_still_receives_bounded_automatic_reconciliation() {
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(root.path().into());
    app.complete_listing(0, 1, Err("transient initial error".into()));
    let mut runtime = Runtime::default();
    runtime.slots[0].worker = None;
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    runtime.poll(&mut app, &mut pool, Duration::from_secs(30));
    assert_eq!(pool.active_count(), 1);
    finish(&mut app, &mut pool);
    assert!(app.panes[0].error.is_none());
    assert!(app.panes[0].directory.is_some());
}

#[test]
fn recovery_before_the_next_ui_poll_does_not_leave_fallback_stuck() {
    let (_root, mut app, mut pool, mut runtime) = prepared();
    runtime.slots[0].failed = true;
    runtime.slots[0].signals.publish(ERROR | DIRTY);
    runtime.slots[0].signals.publish(READY | DIRTY);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(1));
    assert!(!runtime.slots[0].failed);
    assert_eq!(app.operations.notices.len(), 1);
    assert!(app.operations.notices[0].text.contains("recovered"));
}
