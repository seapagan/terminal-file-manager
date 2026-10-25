//! Native Windows manual-refresh policy through the production model and workers.
#![cfg(windows)]
mod windows_manual_terminal;
use windows_manual_terminal::windows_process_metrics::counters;

use std::{
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use terminal_file_manager::{
    app::App,
    fs::{self, IgnoreContext},
    operations::{Operation, Plan},
    watch::Runtime,
    workers::{Pool, Value},
};

fn snapshot(path: &Path) -> fs::Snapshot {
    fs::list(path, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap()
}

struct Browser {
    app: App,
    pool: Pool,
    refresh: Runtime,
    clock: Instant,
    starts: [Vec<Duration>; 2],
    durations: [Vec<f64>; 2],
    in_flight: [Option<Instant>; 2],
    peak_active: usize,
    peak_pending: usize,
}

impl Browser {
    fn new(path: &Path) -> Self {
        let mut app = App::new(path.into());
        for pane in 0..2 {
            app.complete_listing(pane, 1, Ok(snapshot(path)));
        }
        let mut browser = Self {
            app,
            pool: Pool::new(IgnoreContext::isolated()).unwrap(),
            refresh: Runtime::default(),
            clock: Instant::now(),
            starts: Default::default(),
            durations: Default::default(),
            in_flight: Default::default(),
            peak_active: 0,
            peak_pending: 0,
        };
        browser.step();
        browser
    }

    fn step(&mut self) {
        self.poll();
        self.receive();
    }

    fn poll(&mut self) {
        let generations = [self.app.panes[0].generation, self.app.panes[1].generation];
        self.refresh
            .poll(&mut self.app, &mut self.pool, self.clock.elapsed());
        assert_eq!(
            [self.app.panes[0].generation, self.app.panes[1].generation],
            generations,
            "manual Windows mode scheduled an automatic listing"
        );
        self.peak_active = self.peak_active.max(self.pool.active_count());
        self.peak_pending = self.peak_pending.max(self.pool.pending_count());
    }

    fn refresh(&mut self, pane: usize) {
        let request = self.app.request_refresh(pane).unwrap();
        assert!(self.in_flight[pane].is_none());
        self.in_flight[pane] = Some(Instant::now());
        self.starts[pane].push(self.clock.elapsed());
        self.pool.submit(request);
    }

    fn refresh_both(&mut self) {
        for pane in 0..2 {
            self.refresh(pane);
        }
    }

    fn receive(&mut self) {
        if let Some(completion) = self.pool.next(Duration::from_millis(10)) {
            let Value::Listing(result) = completion.value else {
                panic!("listing lane")
            };
            let pane = completion.request.pane;
            if let Some(start) = self.in_flight[pane].take() {
                self.durations[pane].push(start.elapsed().as_secs_f64() * 1000.0);
            }
            self.app
                .complete_listing(pane, completion.request.generation, result);
        }
    }

    fn until(&mut self, predicate: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.step();
            if predicate(&self.app) && self.pool.active_count() == 0 {
                assert_eq!(self.pool.pending_count(), 0);
                return;
            }
            assert!(
                Instant::now() < deadline,
                "explicit refresh did not complete: {:?}",
                self.app.panes.iter().map(|p| &p.error).collect::<Vec<_>>()
            );
        }
    }

    fn observe(&mut self, duration: Duration) {
        let deadline = Instant::now() + duration;
        while Instant::now() < deadline {
            self.step();
        }
        let drain_deadline = Instant::now() + Duration::from_secs(10);
        while self.pool.active_count() > 0 {
            self.receive();
            assert!(Instant::now() < drain_deadline, "listing drain timed out");
        }
    }
}

fn ancestor_rename(level: usize) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("grand/parent/current");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("selected"), "owned").unwrap();
    let mut browser = Browser::new(&path);
    browser.app.panes[0].toggle_mark();
    let target = path.ancestors().nth(level).unwrap();
    let moved = target.with_file_name("moved");
    std::fs::rename(target, &moved).unwrap();
    browser.refresh_both();
    browser.until(|app| app.panes.iter().all(|p| p.error.is_some()));
    assert!(
        browser
            .app
            .panes
            .iter()
            .all(|p| p.path == path && p.current().unwrap().name == "selected")
    );
    assert!(Plan::capture(&browser.app.panes[0], Operation::Trash, None).is_err());
    std::fs::rename(&moved, target).unwrap();
    browser.refresh_both();
    browser.until(|app| app.panes.iter().all(|p| p.error.is_none()));
    assert_eq!(browser.app.panes[0].marks.len(), 1);
    drop(browser);
    std::fs::rename(target, &moved).unwrap();
    std::fs::rename(&moved, target).unwrap();
}

#[test]
fn windows_manual_allows_current_directory_rename_and_recovery() {
    ancestor_rename(0);
}

#[test]
fn windows_manual_allows_grandparent_rename_and_recovery() {
    ancestor_rename(2);
}

#[test]
fn windows_manual_deletion_and_reused_path_require_a_new_directory_context() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("current");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("selected"), "original").unwrap();
    let mut browser = Browser::new(&path);
    browser.app.panes[0].toggle_mark();
    std::fs::rename(
        path.join("selected"),
        root.path().join("preserved-original"),
    )
    .unwrap();
    std::fs::remove_dir(&path).unwrap();
    browser.refresh_both();
    browser.until(|app| app.panes.iter().all(|p| p.error.is_some()));
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("selected"), "replacement").unwrap();
    let generation = browser.app.panes[0].generation;
    browser.refresh_both();
    browser.until(|app| app.panes[0].generation > generation);
    assert!(
        browser
            .app
            .panes
            .iter()
            .all(|p| p.error.is_some() && p.current().unwrap().size == Some(8))
    );
    assert!(Plan::capture(&browser.app.panes[0], Operation::Trash, None).is_err());
    for pane in 0..2 {
        let request = browser.app.request_listing(pane, path.clone());
        browser.pool.submit(request);
    }
    browser.until(|app| app.panes.iter().all(|p| p.error.is_none()));
    assert!(
        browser
            .app
            .panes
            .iter()
            .all(|p| p.marks.is_empty() && p.current().unwrap().size == Some(11))
    );
    assert_eq!(
        std::fs::read(root.path().join("preserved-original")).unwrap(),
        b"original"
    );
    drop(browser);
    std::fs::remove_file(path.join("selected")).unwrap();
    std::fs::remove_dir(path).unwrap();
}

#[test]
fn windows_manual_navigation_discards_old_results_and_refresh_still_works() {
    let root = tempfile::tempdir().unwrap();
    let next = root.path().join("next");
    std::fs::create_dir(&next).unwrap();
    let mut browser = Browser::new(root.path());
    browser.refresh(0);
    let old = browser.app.panes[0].generation;
    let stale = snapshot(root.path());
    let navigation = browser.app.request_listing(0, next.clone());
    browser.pool.submit(navigation);
    assert!(!browser.app.complete_listing(0, old, Ok(stale)));
    browser.until(|app| app.panes[0].path == next);
    std::fs::write(next.join("manual"), "manual update").unwrap();
    browser.refresh(0);
    browser.until(|app| app.panes[0].current().is_some_and(|e| e.name == "manual"));
    assert_eq!(browser.app.panes[1].path, root.path());
    assert!(browser.app.operations.notices.is_empty());
}

#[test]
fn windows_manual_external_changes_wait_for_refresh_and_hidden_panes_stay_independent() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("selected"), "old").unwrap();
    let mut browser = Browser::new(root.path());
    browser.app.panes[0].toggle_mark();
    std::fs::write(root.path().join("selected"), "updated length").unwrap();
    std::fs::write(root.path().join("external"), "new").unwrap();
    browser.observe(Duration::from_millis(2200));
    assert!(browser.starts.iter().all(Vec::is_empty));
    assert!(
        browser
            .app
            .panes
            .iter()
            .all(|p| p.entries.len() == 1 && p.current().unwrap().size == Some(3))
    );
    browser.refresh(0);
    browser.until(|app| app.panes[0].entries.len() == 2);
    assert_eq!(browser.app.panes[0].marks.len(), 1);
    assert_eq!(browser.app.panes[1].entries.len(), 1);
    assert_eq!(browser.app.panes[1].current().unwrap().size, Some(3));
    std::fs::rename(root.path().join("selected"), root.path().join("retained")).unwrap();
    std::fs::write(root.path().join("selected"), "replacement").unwrap();
    browser.refresh(0);
    browser.until(|app| app.panes[0].marks.is_empty());
    assert_eq!(
        std::fs::read(root.path().join("retained")).unwrap(),
        b"updated length"
    );
    browser.refresh(1);
    browser.until(|app| app.panes[1].entries.len() == 3);
    assert!(browser.app.operations.notices.is_empty());
}

#[test]
fn windows_manual_allows_parent_directory_rename_and_recovery() {
    ancestor_rename(1);
}

#[test]
fn windows_manual_stale_display_and_frozen_plan_cannot_copy_a_replacement() {
    use terminal_file_manager::operations::{self, Decision, Options, Outcome};
    let root = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let selected = root.path().join("selected");
    std::fs::write(&selected, "original").unwrap();
    let mut browser = Browser::new(root.path());
    let frozen = Plan::capture(
        &browser.app.panes[0],
        Operation::Copy,
        Some(destination.path().into()),
    )
    .unwrap();
    std::fs::rename(&selected, root.path().join("retained")).unwrap();
    std::fs::write(&selected, "replacement").unwrap();
    browser.observe(Duration::from_millis(2200));
    assert_eq!(browser.app.panes[0].current().unwrap().size, Some(8));
    let stale = Plan::capture(
        &browser.app.panes[0],
        Operation::Copy,
        Some(destination.path().into()),
    )
    .unwrap();
    for plan in [frozen, stale] {
        let result =
            operations::execute(&plan, Options::default(), &AtomicBool::new(false), |_| {
                Decision::Cancel
            });
        assert!(
            result
                .entries
                .iter()
                .all(|entry| entry.outcome == Outcome::Changed)
        );
        assert!(!result.entries.is_empty());
    }
    assert_eq!(std::fs::read(&selected).unwrap(), b"replacement");
    assert_eq!(
        std::fs::read(root.path().join("retained")).unwrap(),
        b"original"
    );
    assert!(
        std::fs::read_dir(destination.path())
            .unwrap()
            .next()
            .is_none()
    );
}

fn isolated_changes(browser: &mut Browser, root: &Path) -> Vec<f64> {
    (0..3)
        .map(|index| {
            let name = format!("isolated-{index}");
            let changed = Instant::now();
            std::fs::write(root.join(&name), "external writer").unwrap();
            browser.refresh_both();
            browser.until(|app| {
                app.panes
                    .iter()
                    .all(|p| p.entries.iter().any(|e| e.name == name.as_str()))
            });
            changed.elapsed().as_secs_f64() * 1000.0
        })
        .collect()
}

fn continuous_changes(browser: &mut Browser, root: &Path) -> usize {
    let file = root.join("item00000");
    let writer = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut writes = 0;
        while Instant::now() < deadline {
            std::fs::write(&file, vec![42; writes + 1]).unwrap();
            writes += 1;
            std::thread::sleep(Duration::from_millis(20));
        }
        std::fs::write(file, "final-owned-writer-state").unwrap();
        writes + 1
    });
    while !writer.is_finished() {
        browser.step();
    }
    let writes = writer.join().unwrap();
    let expected = fs::inspect(&root.join("item00000"));
    let finished = Instant::now();
    browser.refresh_both();
    browser.until(|app| {
        app.panes.iter().all(|p| {
            p.entries.iter().any(|e| {
                e.name == "item00000" && e.size == expected.size && e.modified == expected.modified
            })
        })
    });
    eprintln!(
        "Windows manual continuous writer: {writes} writes, final model convergence {:?}",
        finished.elapsed()
    );
    writes
}

fn manual_comparison(browser: &Browser, root: &Path) -> Duration {
    let manual = Instant::now();
    let expected = snapshot(root);
    let manual_duration = manual.elapsed();
    for pane in &browser.app.panes {
        assert_eq!(
            pane.entries
                .iter()
                .map(|e| (&e.path, (e.size, e.modified)))
                .collect::<std::collections::BTreeMap<_, _>>(),
            expected
                .entries
                .iter()
                .map(|e| (&e.path, (e.size, e.modified)))
                .collect::<std::collections::BTreeMap<_, _>>()
        );
    }
    manual_duration
}

fn measure(entries: usize) {
    let root = tempfile::tempdir().unwrap();
    for index in 0..entries {
        std::fs::write(root.path().join(format!("item{index:05}")), "owned").unwrap();
    }
    let before = counters(std::process::id());
    let mut browser = Browser::new(root.path());
    let idle_before = counters(std::process::id());
    let idle = Instant::now();
    browser.observe(Duration::from_millis(2200));
    let idle_wall = idle.elapsed();
    let idle_after = counters(std::process::id());
    assert!(!browser.app.dual);
    assert!(browser.app.operations.notices.is_empty());
    assert!(browser.starts.iter().all(Vec::is_empty));
    assert_eq!(browser.pool.active_count(), 0);
    eprintln!(
        "Windows manual {entries} entries; debug assertions={}; idle pump={idle_wall:?}, sampled window={:?}, cpu_ms={}, rss_before={}, rss_after={}, handles_before={}, handles_after={}, idle starts={:?}",
        cfg!(debug_assertions),
        idle_before.elapsed_to(&idle_after),
        (idle_after.cpu_ticks - idle_before.cpu_ticks) as f64 / 10000.0,
        idle_before.rss,
        idle_after.rss,
        idle_before.handles,
        idle_after.handles,
        browser.starts
    );
    let latency = isolated_changes(&mut browser, root.path());
    let writes = continuous_changes(&mut browser, root.path());
    let manual_duration = manual_comparison(&browser, root.path());
    assert!(browser.peak_active <= 2);
    assert_eq!(browser.peak_pending, 0);
    eprintln!(
        "Windows manual {entries}: explicit refresh latency_ms={latency:?}, writes={writes}, scan starts_s={:?}, scan durations_ms={:?}, peak active={}, peak pending={}, manual listing={manual_duration:?}",
        browser.starts, browser.durations, browser.peak_active, browser.peak_pending
    );
    drop(browser);
    let released = counters(std::process::id());
    eprintln!(
        "Windows manual {entries}: owned process before={before:?}, after shutdown={released:?}"
    );
    windows_manual_terminal::sample(entries, root.path());
    let moved = root.path().with_extension("released-manual");
    std::fs::rename(root.path(), &moved).unwrap();
    std::fs::rename(&moved, root.path()).unwrap();
}

/// Run on native Windows for release measurements (no CPU pass/fail threshold):
/// `cargo test --release --locked --test windows_polling windows_manual_cost_and_refresh_on_2000_and_10000_entries -- --ignored --exact --nocapture`
#[test]
#[ignore = "Extended Windows CPU/latency measurement; run explicitly in release mode"]
fn windows_manual_cost_and_refresh_on_2000_and_10000_entries() {
    measure(2000);
    measure(10000);
}
