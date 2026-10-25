//! Manual Windows policy exercised with real listings and a controlled clock.
use super::tests::{finish, prepared};
use super::*;

fn manual() -> (tempfile::TempDir, App, Pool, Runtime) {
    let (root, app, pool, _) = prepared();
    (root, app, pool, Runtime::new(Mode::Manual))
}

#[test]
fn manual_policy_never_schedules_hidden_or_visible_panes_at_any_deadline() {
    let (root, mut app, mut pool, mut runtime) = manual();
    let generations = [app.panes[0].generation, app.panes[1].generation];
    std::fs::write(root.path().join("external"), "data").unwrap();
    for seconds in [0, 2, 30, 300, 3600] {
        assert!(!runtime.poll(&mut app, &mut pool, Duration::from_secs(seconds)));
        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.pending_count(), 0);
    }
    assert_eq!(
        [app.panes[0].generation, app.panes[1].generation],
        generations
    );
    assert!(app.panes.iter().all(|pane| pane.entries.is_empty()));
    assert!(!app.dual);
    assert!(app.operations.notices.is_empty());
    assert!(runtime.slots.iter().all(|slot| slot.worker.is_none()));
}

#[test]
fn manual_refresh_updates_only_the_requested_pane_and_preserves_its_mark() {
    let (root, mut app, mut pool, mut runtime) = manual();
    let selected = root.path().join("selected");
    std::fs::write(&selected, "first").unwrap();
    pool.submit(app.request_refresh(0).unwrap());
    finish(&mut app, &mut pool);
    app.panes[0].toggle_mark();
    let hidden_generation = app.panes[1].generation;
    std::fs::write(&selected, "updated size").unwrap();
    runtime.poll(&mut app, &mut pool, Duration::from_secs(60));
    pool.submit(app.request_refresh(0).unwrap());
    finish(&mut app, &mut pool);
    assert_eq!(app.panes[0].current().unwrap().size, Some(12));
    assert_eq!(app.panes[0].marks.len(), 1);
    assert_eq!(app.panes[1].generation, hidden_generation);
    assert!(app.panes[1].entries.is_empty());
    assert_eq!(pool.pending_count(), 0);
}

#[test]
fn manual_mode_ignores_native_error_hints_without_warning_or_retry() {
    let (_root, mut app, mut pool, mut runtime) = manual();
    for slot in &runtime.slots {
        slot.signals.publish(ERROR | DIRTY);
    }
    for seconds in [0, 2, 30, 300] {
        assert!(!runtime.poll(&mut app, &mut pool, Duration::from_secs(seconds)));
    }
    assert!(app.operations.notices.is_empty());
    assert_eq!(pool.active_count(), 0);
    assert!(runtime.slots.iter().all(|slot| !slot.failed));
}

#[test]
fn manual_clock_does_not_cancel_or_queue_work_during_explicit_refresh() {
    let (_root, mut app, mut pool, mut runtime) = manual();
    pool.submit(app.request_refresh(0).unwrap());
    let generation = app.panes[0].generation;
    for seconds in [2, 30, 300] {
        runtime.poll(&mut app, &mut pool, Duration::from_secs(seconds));
    }
    assert_eq!(app.panes[0].generation, generation);
    assert_eq!(pool.active_count(), 1);
    assert_eq!(pool.pending_count(), 0);
    finish(&mut app, &mut pool);
    runtime.poll(&mut app, &mut pool, Duration::from_secs(301));
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn intentional_manual_mode_does_not_retry_an_initial_listing_error() {
    let (root, _, mut pool, mut runtime) = manual();
    let mut app = App::new(root.path().into());
    app.complete_listing(0, 1, Err("initial listing failed".into()));
    let initial_error = app.panes[0].error.clone();
    for seconds in [0, 2, 30, 300] {
        runtime.poll(&mut app, &mut pool, Duration::from_secs(seconds));
    }
    assert_eq!(pool.active_count(), 0);
    assert_eq!(app.panes[0].error, initial_error);
    pool.submit(app.request_refresh(0).unwrap());
    finish(&mut app, &mut pool);
    assert!(app.panes[0].error.is_none());
    assert!(app.panes[0].directory.is_some());
}

#[test]
#[cfg(windows)]
fn windows_default_has_no_native_owner_or_automatic_listing() {
    let (_root, mut app, mut pool, _) = prepared();
    let mut runtime = Runtime::default();
    runtime.poll(&mut app, &mut pool, Duration::from_secs(300));
    assert!(runtime.slots.iter().all(|slot| slot.worker.is_none()));
    assert_eq!(pool.active_count(), 0);
    assert!(app.operations.notices.is_empty());
}
