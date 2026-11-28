use super::Runtime;
use crate::{app::App, owner::windows::Name, ui::Geometry};
use crate::{fs, ui};
use ratatui::layout::Rect;
use std::sync::{Condvar, Mutex, atomic::AtomicUsize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

mod lifecycle;
mod visibility;

fn loaded(count: usize) -> (tempfile::TempDir, App, Geometry) {
    let root = tempfile::tempdir().unwrap();
    for index in 0..count {
        std::fs::write(root.path().join(format!("file-{index:04}")), b"x").unwrap();
    }
    let snapshot = fs::list(
        root.path(),
        &fs::IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let mut app = App::new(root.path().into());
    app.complete_listing(0, 1, Ok(snapshot.clone()));
    app.complete_listing(1, 1, Ok(snapshot));
    for pane in &mut app.panes {
        pane.move_cursor(1, 20);
    }
    app.dual = true;
    let geometry = ui::geometry(&app, Rect::new(0, 0, 240, 40));
    (root, app, geometry)
}

fn until(runtime: &mut Runtime, app: &mut App, geometry: &Geometry, done: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(app) {
        runtime.poll(app, geometry);
        assert!(Instant::now() < deadline, "ownership completion timed out");
        std::thread::yield_now();
    }
}

struct Release(Arc<(Mutex<bool>, Condvar)>);
impl Release {
    fn new() -> Self {
        Self(Arc::new((Mutex::new(false), Condvar::new())))
    }
}
impl Drop for Release {
    fn drop(&mut self) {
        *self.0.0.lock().unwrap() = true;
        self.0.1.notify_all();
    }
}
fn blocked(gate: &Arc<(Mutex<bool>, Condvar)>) {
    let (released, timeout) = gate
        .1
        .wait_timeout_while(
            gate.0.lock().unwrap(),
            Duration::from_secs(20),
            |released| !*released,
        )
        .unwrap();
    assert!(
        *released && !timeout.timed_out(),
        "UI waited for a held native lookup"
    );
}

#[test]
fn repeated_sid_and_two_panes_share_one_read_and_one_name_lookup() {
    let (_root, mut app, geometry) = loaded(1);
    let reads = Arc::new(AtomicUsize::new(0));
    let names = Arc::new(AtomicUsize::new(0));
    let (read_count, name_count) = (reads.clone(), names.clone());
    let mut runtime = Runtime::using(
        move |_, _| {
            read_count.fetch_add(1, Ordering::Relaxed);
            Ok("S-1-5-18".into())
        },
        move |_| {
            name_count.fetch_add(1, Ordering::Relaxed);
            Name::Resolved("DOMAIN\\Alice".into())
        },
    );
    app.request_metadata(0).unwrap();
    until(&mut runtime, &mut app, &geometry, |app| {
        app.panes.iter().all(|pane| {
            pane.entries[0].owner_display(super::super::OwnerDisplay::Names) == "DOMAIN\\Alice"
        })
    });
    assert_eq!(
        app.details[0].entry.as_ref().unwrap().windows_owner,
        app.panes[0].entries[0].windows_owner
    );
    for _ in 0..100 {
        runtime.poll(&mut app, &geometry);
    }
    assert_eq!(reads.load(Ordering::Relaxed), 1);
    assert_eq!(names.load(Ordering::Relaxed), 1);
    let request = app.request_metadata(1).unwrap();
    std::fs::write(request.local_path(), "changed contents with the same owner").unwrap();
    let fresh = fs::inspect(request.local_path());
    app.complete_metadata(&request, fresh, None);
    app.active = 1;
    app.popup = true;
    let geometry = ui::geometry(&app, Rect::new(0, 0, 240, 40));
    runtime.poll(&mut app, &geometry);
    assert_eq!(
        app.details[1].entry.as_ref().unwrap().windows_owner,
        app.panes[1].entries[0].windows_owner
    );
    assert_eq!(reads.load(Ordering::Relaxed), 1);
}

#[test]
fn numeric_views_skip_name_translation_until_metadata_requests_names() {
    use crate::owner::OwnerDisplay;
    let (_root, mut app, geometry) = loaded(1);
    for pane in &mut app.panes {
        pane.owner_display = OwnerDisplay::Numeric;
    }
    let names = Arc::new(AtomicUsize::new(0));
    let calls = names.clone();
    let mut runtime = Runtime::using(
        |_, _| Ok("S-1-5-18".into()),
        move |_| {
            calls.fetch_add(1, Ordering::Relaxed);
            Name::Resolved("DOMAIN\\Alice".into())
        },
    );
    until(&mut runtime, &mut app, &geometry, |app| {
        app.panes
            .iter()
            .all(|pane| pane.entries[0].owner_display(OwnerDisplay::Numeric) == "S-1-5-18")
    });
    assert_eq!(names.load(Ordering::Relaxed), 0);
    app.request_metadata(0).unwrap();
    app.popup = true;
    let mut format = app.metadata_format(true);
    format.owner_display = OwnerDisplay::Names;
    app.metadata_formats[app.active] = Some(format);
    let geometry = ui::geometry(&app, Rect::new(0, 0, 240, 40));
    until(&mut runtime, &mut app, &geometry, |app| {
        app.details[0]
            .entry
            .as_ref()
            .unwrap()
            .owner_display(OwnerDisplay::Names)
            == "DOMAIN\\Alice"
    });
    assert_eq!(names.load(Ordering::Relaxed), 1);
    assert_eq!(app.panes[0].owner_display, OwnerDisplay::Numeric);
}

#[test]
fn ownership_targets_follow_scroll_resize_and_parent_focus_without_extra_rows() {
    let (_root, mut app, mut geometry) = loaded(128);
    geometry.listing_rows[0] = 20;
    for (scroll, cursor, first, count) in [
        (40, Some(45), 39, 20),
        (40, Some(59), 40, 20),
        (999, Some(127), 127, 1),
        (40, Some(0), 0, 20),
        (40, None, 0, 19),
    ] {
        let pane = &mut app.panes[0];
        pane.scroll = scroll;
        pane.cursor = Some(cursor.map_or_else(
            || pane.local_path().parent().unwrap().to_owned(),
            |index| pane.entries[index].local_path().clone(),
        ));
        let targets: Vec<_> = super::view::targets(&app, &geometry)
            .into_iter()
            .filter(|target| target.pane == 0)
            .collect();
        assert_eq!(targets.len(), count);
        assert_eq!(
            targets[0].key.path,
            *app.panes[0].entries[first].local_path()
        );
    }
}

#[test]
fn held_sid_reads_bound_work_without_blocking_scroll_navigation_refresh_or_drop() {
    let (_root, mut app, geometry) = loaded(1000);
    let gate = Release::new();
    let wait = gate.0.clone();
    let (started, entered) = mpsc::channel();
    let mut runtime = Runtime::using(
        move |path, _| {
            started.send(path.to_owned()).unwrap();
            blocked(&wait);
            Ok("S-1-5-18".into())
        },
        |_| Name::Unmapped,
    );
    runtime.poll(&mut app, &geometry);
    for _ in 0..2 {
        entered.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    for _ in 0..1000 {
        app.panes[0].move_cursor(1, 30);
        runtime.poll(&mut app, &geometry);
    }
    assert_eq!(runtime.readers.active.len(), 2);
    assert!(entered.try_recv().is_err());
    assert!(app.request_refresh(0).is_some());
    runtime.poll(&mut app, &geometry);
    app.request_listing(0, app.panes[0].local_path().join("next"));
    runtime.poll(&mut app, &geometry);
    let start = Instant::now();
    drop(runtime);
    assert!(start.elapsed() < Duration::from_secs(2));
    drop(gate);
}

#[test]
fn held_name_translation_keeps_numeric_sid_available_for_the_whole_viewport() {
    let (_root, mut app, geometry) = loaded(8);
    let gate = Release::new();
    let wait = gate.0.clone();
    let (started, entered) = mpsc::channel();
    let mut runtime = Runtime::using(
        |_, _| Ok("S-1-5-18".into()),
        move |_| {
            started.send(()).unwrap();
            blocked(&wait);
            Name::Resolved("DOMAIN\\Alice".into())
        },
    );
    until(&mut runtime, &mut app, &geometry, |app| {
        app.panes[0]
            .entries
            .iter()
            .all(|entry| entry.owner_display(super::super::OwnerDisplay::Numeric) == "S-1-5-18")
    });
    entered.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(runtime.namers.active.len(), 1);
    assert!(entered.try_recv().is_err());
    assert!(app.panes[0].entries.iter().all(|entry| {
        entry
            .owner_display(super::super::OwnerDisplay::Names)
            .ends_with("(resolving)")
    }));
    let pending_width = app.panes[0].measurements.owner;
    drop(gate);
    until(&mut runtime, &mut app, &geometry, |app| {
        app.panes[0]
            .entries
            .iter()
            .all(|entry| entry.owner_display(super::super::OwnerDisplay::Names) == "DOMAIN\\Alice")
    });
    assert_eq!(app.panes[0].measurements.owner, 12);
    assert!(app.panes[0].measurements.owner < pending_width);
}

#[test]
fn metadata_request_precedes_rows_and_generation_changes_reject_late_sid_reads() {
    let (root, mut app, _) = loaded(10);
    app.panes[0].cursor = Some(app.panes[0].entries[5].local_path().clone());
    app.popup = true;
    app.request_metadata(0).unwrap();
    let geometry = ui::geometry(&app, Rect::new(0, 0, 240, 40));
    let wanted = app.panes[0].cursor.clone().unwrap();
    let gate = Release::new();
    let wait = gate.0.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let (started, entered) = mpsc::channel();
    let mut runtime = Runtime::using(
        move |path, _| {
            let call = counter.fetch_add(1, Ordering::Relaxed);
            started.send(path.to_owned()).unwrap();
            if call == 0 {
                blocked(&wait);
            }
            Ok(if call == 0 { "S-1-5-18" } else { "S-1-5-19" }.into())
        },
        |_| Name::Unmapped,
    );
    runtime.poll(&mut app, &geometry);
    // Both workers may start in either OS scheduling order; the first submission
    // must still be the metadata target (recorded in the in-flight slots).
    assert_eq!(runtime.readers.active[0].key.path, wanted);
    entered.recv_timeout(Duration::from_secs(10)).unwrap();
    let request = app.request_refresh(0).unwrap();
    runtime.poll(&mut app, &geometry);
    let snapshot = fs::list(
        root.path(),
        &fs::IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    app.complete_listing(0, request.generation, Ok(snapshot));
    until(&mut runtime, &mut app, &geometry, |app| {
        app.panes[0].entries[5].owner_display(super::super::OwnerDisplay::Numeric) == "S-1-5-19"
    });
    drop(gate);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !runtime.readers.active.is_empty() {
        runtime.poll(&mut app, &geometry);
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(
        app.panes[0].entries[5].owner_display(super::super::OwnerDisplay::Numeric),
        "S-1-5-19"
    );
}

#[test]
fn failed_reads_and_failed_names_are_cached_without_losing_available_sid() {
    let (_root, mut app, geometry) = loaded(2);
    let reads = Arc::new(AtomicUsize::new(0));
    let count = reads.clone();
    let mut runtime = Runtime::using(
        move |path, _| {
            count.fetch_add(1, Ordering::Relaxed);
            if path.ends_with("file-0000") {
                Err("access denied".into())
            } else {
                Ok("S-1-5-18".into())
            }
        },
        |_| Name::Failed("domain unavailable".into()),
    );
    until(&mut runtime, &mut app, &geometry, |app| {
        app.panes[0].entries[0].windows_owner.error() == Some("access denied")
            && app.panes[0].entries[1].windows_owner.error() == Some("domain unavailable")
    });
    for _ in 0..100 {
        runtime.poll(&mut app, &geometry);
    }
    assert_eq!(reads.load(Ordering::Relaxed), 2);
    assert_eq!(
        app.panes[0].entries[1].owner_display(super::super::OwnerDisplay::Numeric),
        "S-1-5-18"
    );
}
