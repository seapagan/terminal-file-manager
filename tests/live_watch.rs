//! Real native backend plus the same watch/model/worker bridge used by the TUI.
use std::{
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use terminal_file_manager::{
    app::App,
    fs::{self, IgnoreContext},
    watch::Runtime,
    workers::{Pool, Value},
};

#[cfg(unix)]
struct RestorePermissions(std::path::PathBuf, std::fs::Permissions);

#[cfg(unix)]
impl Drop for RestorePermissions {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, self.1.clone());
    }
}

fn snapshot(path: &Path) -> fs::Snapshot {
    fs::list(path, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap()
}

fn assert_manual_listing_matches(app: &App, path: &Path) {
    use std::collections::BTreeMap;
    let started = Instant::now();
    let manual = snapshot(path);
    eprintln!(
        "Manual {}-row listing: {:?}",
        manual.entries.len(),
        started.elapsed()
    );
    let expected: BTreeMap<_, _> = manual.entries.iter().map(|e| (&e.path, e.size)).collect();
    for pane in &app.panes {
        let actual: BTreeMap<_, _> = pane.entries.iter().map(|e| (&e.path, e.size)).collect();
        assert_eq!(actual, expected);
    }
}

fn loaded(path: &Path) -> App {
    let mut app = App::new(path.into());
    for pane in 0..2 {
        app.complete_listing(
            pane,
            1,
            Ok(fs::list(path, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap()),
        );
    }
    app
}

fn converge(app: &mut App, pool: &mut Pool, watch: &mut Runtime, started: Instant, expected: &str) {
    until(app, pool, watch, started, |app| {
        app.panes
            .iter()
            .all(|p| p.entries.iter().any(|e| e.name == expected))
    });
}

fn until(
    app: &mut App,
    pool: &mut Pool,
    watch: &mut Runtime,
    started: Instant,
    done: impl Fn(&App) -> bool,
) {
    // Windows runs the same filesystem/safety cases after deliberate refresh;
    // Linux/macOS continue to prove native automatic convergence.
    #[cfg(windows)]
    for pane in 0..2 {
        if let Some(request) = app.request_refresh(pane) {
            pool.submit(request);
        }
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let (mut active, mut queued) = (0, 0);
    loop {
        watch.poll(app, pool, started.elapsed());
        active = active.max(pool.active_count());
        queued = queued.max(pool.pending_count());
        if let Some(completion) = pool.next(Duration::from_millis(10))
            && let Value::Listing(result) = completion.value
        {
            app.complete_listing(
                completion.request.pane,
                completion.request.generation,
                result,
            );
        }
        if done(app) && pool.active_count() == 0 && pool.pending_count() == 0 {
            eprintln!("Watch convergence: peak active lanes {active}, pending lanes {queued}");
            return;
        }
        assert!(
            Instant::now() < deadline,
            "native watcher did not converge; errors: {:?}",
            app.panes.iter().map(|p| &p.error).collect::<Vec<_>>()
        );
    }
}

#[test]
fn native_writes_rename_delete_and_storm_converge_with_bounded_scans() {
    let root = tempfile::tempdir().unwrap();
    let mut app = loaded(root.path());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    std::fs::write(root.path().join("ready"), "data").unwrap();
    converge(&mut app, &mut pool, &mut watch, started, "ready");
    let baseline = app.panes[0].generation;
    let changed = Instant::now();
    for index in 0..1000 {
        std::fs::write(root.path().join(format!("item{index:04}")), "content").unwrap();
    }
    std::fs::rename(root.path().join("item0000"), root.path().join("renamed")).unwrap();
    std::fs::remove_file(root.path().join("item0001")).unwrap();
    std::fs::write(root.path().join("ready"), "new-length").unwrap();
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| {
            p.entries.len() == 1000
                && p.entries.iter().any(|e| e.name == "renamed")
                && p.entries
                    .iter()
                    .any(|e| e.name == "ready" && e.size == Some(10))
                && !p
                    .entries
                    .iter()
                    .any(|e| e.name == "item0000" || e.name == "item0001")
        })
    });
    let scans = app.panes[0].generation - baseline;
    assert!(scans <= 5, "unbounded scans: {scans}");
    eprintln!(
        "1000-file burst: {scans} scans/pane, convergence {:?}",
        changed.elapsed()
    );
    assert!(pool.pending_count() <= 2);
    assert_manual_listing_matches(&app, root.path());
}

#[test]
fn native_directory_replacement_retains_last_good_rows_until_reopen() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("current");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("original"), "old").unwrap();
    let mut app = loaded(&path);
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    std::fs::write(path.join("ready"), "data").unwrap();
    converge(&mut app, &mut pool, &mut watch, started, "ready");
    std::fs::rename(&path, root.path().join("previous")).unwrap();
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("replacement"), "new").unwrap();
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| p.error.is_some())
    });
    assert!(
        app.panes
            .iter()
            .all(|p| p.entries.iter().any(|e| e.name == "original"))
    );
    assert!(
        app.panes
            .iter()
            .all(|p| !p.entries.iter().any(|e| e.name == "replacement"))
    );
    for pane in 0..2 {
        pool.submit(app.request_listing(pane, path.clone()));
    }
    converge(&mut app, &mut pool, &mut watch, started, "replacement");
}

#[cfg(unix)]
#[test]
fn native_symlink_spelling_observes_physical_directory_without_retargeting() {
    let root = tempfile::tempdir().unwrap();
    let physical = root.path().join("physical");
    let alias = root.path().join("alias");
    std::fs::create_dir(&physical).unwrap();
    std::os::unix::fs::symlink(&physical, &alias).unwrap();
    let mut app = loaded(&alias);
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    std::fs::write(physical.join("ready"), "data").unwrap();
    converge(&mut app, &mut pool, &mut watch, started, "ready");
    std::fs::write(physical.join("physical-write"), "data").unwrap();
    converge(&mut app, &mut pool, &mut watch, started, "physical-write");
    assert!(app.panes.iter().all(|p| p.path == alias));
}

#[test]
fn native_metadata_changes_update_the_selected_detail_without_cursor_motion() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("selected");
    std::fs::write(&path, "old").unwrap();
    let original = std::fs::metadata(&path).unwrap().permissions();
    let mut app = loaded(root.path());
    app.popup = true;
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| p.generation > 1)
    });
    app.request_metadata(0).unwrap();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&path, readonly).unwrap();
    let expected = fs::inspect(&path);
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| {
            p.current()
                .is_some_and(|e| e.mode == expected.mode && e.attributes == expected.attributes)
        })
    });
    app.request_metadata(0).unwrap();
    assert_eq!(app.details[0].entry.as_ref().unwrap().mode, expected.mode);
    assert_eq!(
        app.details[0].entry.as_ref().unwrap().attributes,
        expected.attributes
    );
    assert_eq!(app.panes[0].cursor.as_deref(), Some(path.as_path()));
    assert!(app.popup);
    std::fs::set_permissions(path, original).unwrap();
}

#[test]
fn native_same_name_file_replacement_drops_the_mark_and_keeps_the_cursor() {
    let root = tempfile::tempdir().unwrap();
    let selected = root.path().join("selected");
    std::fs::write(&selected, "old").unwrap();
    let mut app = loaded(root.path());
    app.panes[0].toggle_mark();
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| p.generation > 1)
    });
    std::fs::rename(&selected, root.path().join("preserved-original")).unwrap();
    std::fs::write(&selected, "replacement").unwrap();
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| {
            p.entries
                .iter()
                .any(|e| e.name == "selected" && e.size == Some(11))
        })
    });
    assert_eq!(app.panes[0].cursor.as_deref(), Some(selected.as_path()));
    assert!(app.panes[0].marks.is_empty());
    assert_eq!(
        std::fs::read(root.path().join("preserved-original")).unwrap(),
        b"old"
    );
}

#[test]
fn native_parent_rename_keeps_the_old_context_and_recovers_when_restored() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("parent");
    let path = parent.join("current");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("selected"), "data").unwrap();
    let mut app = loaded(&path);
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| p.generation > 1)
    });
    let renamed = root.path().join("renamed-parent");
    std::fs::rename(&parent, &renamed).unwrap();
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| p.error.is_some())
    });
    assert!(
        app.panes
            .iter()
            .all(|p| p.path == path && p.current().unwrap().name == "selected")
    );
    std::fs::rename(renamed, parent).unwrap();
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| p.error.is_none())
    });
}

#[cfg(unix)]
#[test]
fn native_permission_loss_keeps_last_good_rows_and_recovers_without_navigation() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("current");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("selected"), "data").unwrap();
    let mut app = loaded(&path);
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| p.generation > 1)
    });
    let restore = RestorePermissions(
        path.clone(),
        std::fs::metadata(&path).unwrap().permissions(),
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| p.error.is_some())
    });
    assert!(
        app.panes
            .iter()
            .all(|p| p.current().unwrap().name == "selected")
    );
    drop(restore);
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| p.error.is_none())
    });
}

#[test]
#[cfg(not(windows))]
fn native_reads_and_unrelated_parent_chatter_do_not_schedule_scans() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("current");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("selected"), "data").unwrap();
    let mut app = loaded(&path);
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| p.generation > 1)
    });
    let baseline = [app.panes[0].generation, app.panes[1].generation];
    for index in 0..100 {
        std::fs::read(path.join("selected")).unwrap();
        fs::list(&path, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap();
        std::fs::write(root.path().join(format!("sibling-{index}")), "unrelated").unwrap();
        watch.poll(&mut app, &mut pool, started.elapsed());
        assert!(pool.next(Duration::from_millis(10)).is_none());
    }
    assert_eq!([app.panes[0].generation, app.panes[1].generation], baseline);
}

fn copy_fixture() -> (
    tempfile::TempDir,
    App,
    terminal_file_manager::operations::Plan,
) {
    use terminal_file_manager::operations::{Operation, Plan};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(source.join("payload"), vec![42; 4 * 1024 * 1024]).unwrap();
    std::fs::write(destination.join("payload"), "existing destination").unwrap();
    let mut app = loaded(&source);
    let navigation = app.request_listing(1, destination.clone());
    app.complete_listing(1, navigation.generation, Ok(snapshot(&destination)));
    let mut plan =
        Plan::capture(&app.panes[0], Operation::Copy, Some(destination.clone())).unwrap();
    plan.capture_destination(&app.panes[1]).unwrap();
    (root, app, plan)
}

fn await_conflict(
    queue: &mut terminal_file_manager::operations::Queue,
) -> std::sync::mpsc::SyncSender<terminal_file_manager::operations::Decision> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "copy conflict timed out");
        if let Some(terminal_file_manager::operations::Update::Conflict { reply, .. }) =
            queue.next(Duration::from_millis(10))
        {
            return reply;
        }
    }
}

#[test]
fn native_external_writes_during_copy_do_not_retarget_the_frozen_job() {
    use terminal_file_manager::operations::{Decision, Outcome, Queue, Update};
    let (_root, mut app, plan) = copy_fixture();
    let source = app.panes[0].path.clone();
    let destination = app.panes[1].path.clone();
    let frozen = plan.operands[0].clone();
    let mut queue = Queue::new().unwrap();
    let id = queue.submit(plan, Default::default()).unwrap();
    let reply = await_conflict(&mut queue);
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    std::fs::write(source.join("external"), "source external").unwrap();
    std::fs::write(destination.join("external"), "destination external").unwrap();
    converge(&mut app, &mut pool, &mut watch, started, "external");
    app.panes[0].move_cursor(-1, 20);
    reply.send(Decision::Replace).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let record = loop {
        assert!(Instant::now() < deadline, "copy acknowledgement timed out");
        if let Some(Update::Finished {
            id: finished,
            record,
        }) = queue.next(Duration::from_millis(10))
        {
            assert_eq!(finished, id);
            break record;
        }
    };
    assert_eq!(record.entries.len(), 1);
    assert_eq!(record.entries[0].source, frozen.path);
    assert_eq!(record.entries[0].outcome, Outcome::Completed);
    assert_eq!(
        std::fs::read(destination.join("external")).unwrap(),
        b"destination external"
    );
    assert_eq!(
        std::fs::metadata(destination.join("payload"))
            .unwrap()
            .len(),
        4 * 1024 * 1024
    );
}

#[cfg(windows)]
#[test]
fn native_windows_junction_and_case_spelling_remain_the_selected_context() {
    let root = tempfile::tempdir().unwrap();
    let physical = root.path().join("physical");
    let alias = root.path().join("junction");
    std::fs::create_dir(&physical).unwrap();
    let status = std::process::Command::new("cmd")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&alias)
        .arg(&physical)
        .status()
        .unwrap();
    assert!(status.success());
    let spelling = root.path().join("JuNcTiOn");
    let mut app = loaded(&spelling);
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    std::fs::write(physical.join("ready"), "data").unwrap();
    converge(&mut app, &mut pool, &mut watch, started, "ready");
    std::fs::write(physical.join("later"), "data").unwrap();
    converge(&mut app, &mut pool, &mut watch, started, "later");
    assert!(app.panes.iter().all(|p| p.path == spelling));
    std::fs::remove_dir(alias).unwrap();
}

#[test]
#[cfg(not(windows))]
fn native_external_create_refreshes_both_panes_without_input_including_registration_gap() {
    let root = tempfile::tempdir().unwrap();
    let mut app = loaded(root.path());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    std::fs::write(root.path().join("registration-gap"), "data").unwrap();
    converge(&mut app, &mut pool, &mut watch, started, "registration-gap");
    std::fs::write(root.path().join("external-create"), "data").unwrap();
    converge(&mut app, &mut pool, &mut watch, started, "external-create");
    assert!(!app.dual);
}
