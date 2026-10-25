use super::tests::{finish, prepared};
use super::*;
use crate::operations::{Operation, Plan};
use crate::{
    fs::{self, IgnoreContext},
    workers::Value,
};
use std::sync::atomic::AtomicBool;

fn fail_pending(app: &mut App, pool: &mut Pool) {
    while pool.active_count() != 0 {
        let completion = pool.next(Duration::from_secs(5)).unwrap();
        let Value::Listing(result) = completion.value else {
            panic!("listing lane")
        };
        app.complete_listing(
            completion.request.pane,
            completion.request.generation,
            if completion.request.pane == 0 {
                Err("controlled read failure".into())
            } else {
                result
            },
        );
    }
}

#[test]
fn failed_reconciliation_retries_promptly_and_only_success_unblocks_operations() {
    let (root, mut app, mut pool, mut runtime) = prepared();
    std::fs::write(root.path().join("selected"), "data").unwrap();
    pool.submit(app.request_refresh(0).unwrap());
    finish(&mut app, &mut pool);
    let failed = app.request_reconciliation(0).unwrap();
    app.complete_listing(0, failed.generation, Err("transient read failure".into()));
    runtime.poll(&mut app, &mut pool, Duration::ZERO);
    assert!(Plan::capture(&app.panes[0], Operation::Delete, None).is_err());
    runtime.poll(&mut app, &mut pool, Duration::from_millis(999));
    assert_eq!(pool.active_count(), 0);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(1));
    assert_eq!(pool.active_count(), 1);
    assert!(app.panes[0].error.is_some());
    finish(&mut app, &mut pool);
    assert!(app.panes[0].error.is_none());
    assert!(Plan::capture(&app.panes[0], Operation::Delete, None).is_ok());
    assert!(!app.complete_listing(0, failed.generation, Err("late failure".into())));
    assert!(app.panes[0].error.is_none());
}

#[test]
fn degraded_scan_intervals_grow_to_thirty_seconds_despite_repeated_errors() {
    let (_root, mut app, mut pool, mut runtime) = prepared();
    let mut starts = Vec::new();
    for millis in (0..=90000).step_by(100) {
        runtime.slots[0].signals.publish(ERROR | DIRTY);
        let generation = app.panes[0].generation;
        runtime.poll(&mut app, &mut pool, Duration::from_millis(millis));
        if app.panes[0].generation != generation {
            starts.push(millis);
        }
        finish(&mut app, &mut pool);
    }
    assert_eq!(starts, [0, 2000, 6000, 14000, 30000, 60000, 90000]);
    assert_eq!(app.operations.notices.len(), 1);
}

#[test]
fn persistent_listing_errors_cap_retries_even_under_dirty_and_error_hints() {
    let (_root, mut app, mut pool, mut runtime) = prepared();
    let request = app.request_reconciliation(0).unwrap();
    app.complete_listing(0, request.generation, Err("initial failure".into()));
    let mut starts = Vec::new();
    for millis in (0..=92000).step_by(100) {
        runtime.slots[0]
            .signals
            .publish(if millis % 200 == 0 { DIRTY } else { ERROR });
        let generation = app.panes[0].generation;
        runtime.poll(&mut app, &mut pool, Duration::from_millis(millis));
        if app.panes[0].generation != generation {
            starts.push(millis);
        }
        assert!(pool.pending_count() == 0 && pool.active_count() <= 2);
        fail_pending(&mut app, &mut pool);
        assert!(app.panes[0].error.is_some());
    }
    // Retry delays begin on the first idle poll after each failed completion.
    assert_eq!(starts, [1000, 3100, 7200, 15300, 31400, 61500, 91600]);
}

#[test]
fn retry_result_cannot_override_newer_navigation_or_clear_its_error() {
    let (_root, mut app, mut pool, mut runtime) = prepared();
    let request = app.request_reconciliation(0).unwrap();
    app.complete_listing(0, request.generation, Err("transient failure".into()));
    runtime.poll(&mut app, &mut pool, Duration::ZERO);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(1));
    let retry = pool.next(Duration::from_secs(5)).unwrap();
    let next = tempfile::tempdir().unwrap();
    let navigation = app.request_listing(0, next.path().into());
    let Value::Listing(result) = retry.value else {
        panic!("listing lane")
    };
    assert!(!app.complete_listing(0, retry.request.generation, result));
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
    assert!(!app.complete_listing(
        0,
        retry.request.generation,
        Err("late retry failure".into())
    ));
    runtime.poll(&mut app, &mut pool, Duration::from_secs(2));
    finish(&mut app, &mut pool);
    assert_eq!(app.panes[0].path, next.path());
    assert!(app.panes[0].error.is_none());
}

#[test]
fn retry_cannot_adopt_a_replacement_directory_or_unblock_its_operations() {
    let (root, mut app, mut pool, mut runtime) = prepared();
    std::fs::write(root.path().join("selected"), "original").unwrap();
    pool.submit(app.request_refresh(0).unwrap());
    finish(&mut app, &mut pool);
    let request = app.request_reconciliation(0).unwrap();
    app.complete_listing(0, request.generation, Err("transient failure".into()));
    let preserved = tempfile::tempdir().unwrap();
    std::fs::rename(root.path(), preserved.path().join("original")).unwrap();
    std::fs::create_dir(root.path()).unwrap();
    std::fs::write(root.path().join("replacement"), "other").unwrap();
    runtime.poll(&mut app, &mut pool, Duration::ZERO);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(1));
    finish(&mut app, &mut pool);
    assert_eq!(app.panes[0].current().unwrap().name, "selected");
    assert!(app.panes[0].error.as_ref().unwrap().contains("identity"));
    assert!(Plan::capture(&app.panes[0], Operation::Delete, None).is_err());
    assert_eq!(
        std::fs::read(preserved.path().join("original/selected")).unwrap(),
        b"original"
    );
}
