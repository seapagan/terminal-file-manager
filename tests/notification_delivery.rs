//! Passive diagnostics, renewed failures and preference application retain their routes.
use std::{fs, sync::atomic::AtomicBool, time::Duration};
use terminal_file_manager::{
    app::App,
    fs::{IgnoreContext, Snapshot, inspect, list},
    input::{Bindings, Controller},
    notifications::Severity,
    settings::Settings,
};

fn snapshot(root: &std::path::Path) -> Snapshot {
    list(root, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap()
}

fn loaded() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("entry"), "owned").unwrap();
    let mut app = App::new(root.path().into());
    assert!(app.complete_listing(0, 1, Ok(snapshot(root.path()))));
    (root, app)
}

#[test]
fn metadata_entry_errors_stay_inline_without_notifications() {
    let (_root, mut app) = loaded();
    let request = app.request_metadata(0).unwrap();
    fs::remove_file(&request.path).unwrap();
    let entry = inspect(&request.path);
    let error = entry.error.clone().unwrap();
    let status = app.status.clone();
    assert!(app.complete_metadata(&request, entry, None));
    assert_eq!(app.details[0].entry.as_ref().unwrap().error, Some(error));
    assert!(!app.details[0].canonical_pending);
    assert_eq!(app.status, status);
    assert!(app.notifications.history.is_empty());
    assert!(app.notifications.visible().is_empty());
}

#[test]
fn metadata_worker_failures_notify_but_stale_failures_do_not() {
    let (_root, mut app) = loaded();
    let request = app.request_metadata(0).unwrap();
    let mut stale = request.clone();
    stale.generation += 1;
    assert!(!app.complete_failure(&stale, "stale worker panic".into()));
    assert!(app.notifications.history.is_empty());
    assert!(app.complete_failure(&request, "metadata worker failed".into()));
    assert_eq!(app.notifications.history.len(), 1);
    let event = &app.notifications.history[0];
    assert_eq!(event.severity, Severity::Error);
    assert!(event.text.contains("metadata worker failed"));
    assert_eq!(event.job, None);
    assert_eq!(
        app.details[0].entry.as_ref().unwrap().error.as_deref(),
        Some("metadata worker failed")
    );
}

#[test]
fn listing_warnings_suppress_unchanged_refresh_but_notify_after_recovery() {
    let (root, mut app) = loaded();
    for (seconds, warning, count) in [(0, true, 1), (3, true, 1), (6, false, 1), (9, true, 2)] {
        app.now = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(seconds);
        app.notifications.advance(Duration::from_secs(seconds));
        let request = app.request_refresh(0).unwrap();
        let mut result = snapshot(root.path());
        if warning {
            result.errors.push("Ignore rules unavailable".into());
        }
        assert!(app.complete_listing(0, request.generation, Ok(result)));
        assert_eq!(app.notifications.history.len(), count);
        assert_eq!(app.panes[0].warnings.is_empty(), !warning);
    }
    assert!(app.notifications.history.iter().all(|event| {
        event.severity == Severity::Warning && event.text.contains("Ignore rules unavailable")
    }));
}

#[test]
fn listing_errors_notify_again_after_successful_recovery() {
    let (root, mut app) = loaded();
    for (seconds, failure, count) in [(0, true, 1), (3, true, 1), (6, false, 1), (9, true, 2)] {
        app.now = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(seconds);
        app.notifications.advance(Duration::from_secs(seconds));
        let request = app.request_refresh(0).unwrap();
        let result = if failure {
            Err("Directory read denied".into())
        } else {
            Ok(snapshot(root.path()))
        };
        assert!(app.complete_listing(0, request.generation, result));
        assert_eq!(app.notifications.history.len(), count);
        assert_eq!(app.panes[0].error.is_some(), failure);
    }
    assert!(app.notifications.history.iter().all(|event| {
        event.severity == Severity::Error && event.text.contains("Directory read denied")
    }));
}

#[test]
fn toast_settings_share_startup_controller_and_preference_application() {
    let root = tempfile::tempdir().unwrap();
    let initial =
        Settings::parse("toast_placement = 'top-left'\ntoast_expiry_seconds = 12").unwrap();
    let mut startup = App::new(root.path().into());
    initial.apply(&mut startup);
    let mut controller = Controller::new(
        App::new(root.path().into()),
        Bindings::new(&Default::default()).unwrap(),
        initial.clone(),
    );
    for app in [&startup, &controller.app] {
        assert_eq!(app.notifications.placement, initial.toast_placement);
        assert_eq!(app.notifications.expiry_seconds, 12);
    }
    let changed =
        Settings::parse("toast_placement = 'bottom-left'\ntoast_expiry_seconds = 7").unwrap();
    changed.apply(&mut controller.app);
    assert_eq!(
        controller.app.notifications.placement,
        changed.toast_placement
    );
    assert_eq!(controller.app.notifications.expiry_seconds, 7);
}

#[test]
fn renewed_listing_failure_within_group_window_publishes_a_fresh_presentation() {
    let (root, mut app) = loaded();
    let request = app.request_refresh(0).unwrap();
    assert!(app.complete_listing(0, request.generation, Err("Directory read denied".into())));
    app.notifications.dismiss_toasts();
    let request = app.request_refresh(0).unwrap();
    assert!(app.complete_listing(0, request.generation, Ok(snapshot(root.path()))));
    let request = app.request_refresh(0).unwrap();
    assert!(app.complete_listing(0, request.generation, Err("Directory read denied".into())));
    assert_eq!(app.notifications.history.len(), 1);
    assert_eq!(app.notifications.history[0].occurrences, 2);
    assert_eq!(app.notifications.visible(), [0]);
}
