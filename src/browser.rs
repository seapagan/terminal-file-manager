//! Runtime orchestration: terminal events and bounded worker results meet here.
use crate::{
    app::{App, Request, RequestKind},
    color::Theme,
    fs::IgnoreContext,
    input::{Bindings, Controller},
    settings::{Settings, Store},
    terminal::{self, NativeTerminal},
    ui,
    workers::{Pool, Value},
};
use std::{
    io::{self, IsTerminal},
    path::Path,
    time::{Duration, Instant},
};

/// Browse an explicit native directory; failures never substitute another target.
///
/// # Errors
/// Reports path/configuration/nonterminal/setup/application/persistence errors.
pub fn run(path: &Path) -> Result<(), String> {
    run_startup(&crate::startup::Startup {
        paths: vec![path.into()],
        ..Default::default()
    })
}

/// Browse zero, one or two explicit native locations with optional dual layout.
///
/// # Errors
/// Reports either selected path's error before terminal setup; never substitutes a path.
pub fn run_startup(options: &crate::startup::Startup) -> Result<(), String> {
    let mut store = crate::settings::config_path()
        .map(Store::load)
        .transpose()?;
    let settings = store
        .as_ref()
        .map_or_else(Settings::default, |store| store.settings.clone());
    let app = options.prepare(&settings)?;
    if options.save_session {
        return crate::startup::session_store()?.save(&app);
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("interactive browsing requires a terminal on stdin and stdout".into());
    }
    browse(app, settings, &mut store)
}

fn browse(app: App, settings: Settings, store: &mut Option<Store>) -> Result<(), String> {
    let bindings = Bindings::new(&settings.bindings)?;
    let mut controller = Controller::new(app, bindings, settings.clone());
    let theme = Theme::for_stdout();
    let mut pool = Pool::new(IgnoreContext::default()).map_err(|e| e.to_string())?;
    for pane in 0..2 {
        pool.submit(Request {
            pane,
            generation: 1,
            path: controller.app.panes[pane].path.clone(),
            kind: RequestKind::Listing,
        });
    }
    terminal::with_terminal(settings.mouse, |terminal| {
        drive(terminal, &mut controller, &mut pool, &theme)
    })
    .map_err(|e| e.to_string())?;
    if controller.save_session {
        crate::startup::session_store()?.save(&controller.app)?;
    }
    if controller.preferences_changed {
        let Some(store) = store.as_mut() else {
            return Err("preferences changed but no user config directory is available".into());
        };
        store.settings = controller.preferences;
        store.save()?;
    }
    Ok(())
}

fn drive(
    terminal: &mut NativeTerminal,
    controller: &mut Controller,
    pool: &mut Pool,
    theme: &Theme,
) -> io::Result<()> {
    let started = Instant::now();
    let mut metadata: [Option<(Request, Instant)>; 2] = [None, None];
    let mut dirty = true;
    let mut geometry = ui::geometry(&controller.app, terminal.size()?.into());
    while !controller.quit {
        dirty |= tick(&mut controller.app);
        dirty |= collect(controller, pool);
        dirty |= schedule_metadata(controller, pool, &mut metadata);
        if dirty {
            terminal.draw(|frame| {
                geometry = ui::render(frame, &controller.app, theme);
            })?;
            dirty = false;
        }
        if crossterm::event::poll(Duration::from_millis(16))? {
            let requests =
                controller.handle(crossterm::event::read()?, &geometry, started.elapsed());
            for request in requests {
                pool.submit(request);
            }
            dirty = true;
        }
    }
    Ok(())
}

fn tick(app: &mut App) -> bool {
    let now = std::time::SystemTime::now();
    let elapsed = now.duration_since(app.now).unwrap_or(Duration::MAX);
    if elapsed < Duration::from_secs(1) {
        return false;
    }
    app.now = now;
    app.panes.iter().any(|pane| pane.fuzzy_timestamps)
        || app
            .metadata_formats
            .iter()
            .flatten()
            .any(|format| format.fuzzy_timestamps)
}

fn collect(controller: &mut Controller, pool: &mut Pool) -> bool {
    let mut changed = false;
    for _ in 0..6 {
        let Some(completion) = pool.next(Duration::ZERO) else {
            break;
        };
        let request = completion.request;
        changed = true;
        match completion.value {
            Value::Listing(result) => {
                controller
                    .app
                    .complete_listing(request.pane, request.generation, result);
            }
            Value::Metadata(entry, canonical) => {
                controller
                    .app
                    .complete_metadata(&request, *entry, canonical);
            }
            Value::Totals(result) => {
                controller.app.complete_totals(&request, result);
            }
            Value::Failed(error) => {
                controller.app.complete_failure(&request, error);
            }
        }
    }
    changed
}

fn schedule_metadata(
    controller: &mut Controller,
    pool: &mut Pool,
    pending: &mut [Option<(Request, Instant)>; 2],
) -> bool {
    let mut changed = false;
    for (pane, slot) in pending.iter_mut().enumerate() {
        if let Some(request) = controller.app.request_metadata(pane) {
            pool.cancel(pane, RequestKind::Metadata);
            pool.cancel(pane, RequestKind::Totals);
            *slot = Some((request, Instant::now()));
            changed = true;
        }
        if !controller.app.details[pane].counting {
            pool.cancel(pane, RequestKind::Totals);
        }
        let visible =
            (controller.app.sidebar || controller.app.popup) && pane == controller.app.active;
        if visible
            && slot
                .as_ref()
                .is_some_and(|(_, time)| time.elapsed() >= Duration::from_millis(80))
            && let Some((request, _)) = slot.take()
        {
            pool.submit(request);
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fs, input::Action};
    use std::sync::{Arc, Mutex, atomic::AtomicBool, mpsc};

    fn counting_controller(root: &Path) -> Controller {
        let directory = root.join("directory");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("file"), "1234").unwrap();
        let mut app = App::new(root.to_path_buf());
        app.complete_listing(
            0,
            1,
            Ok(fs::list(root, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap()),
        );
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Settings::default(),
        )
    }

    fn collect_until_idle(controller: &mut Controller, pool: &mut Pool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut changed = false;
        while pool.active_count() != 0 {
            assert!(Instant::now() < deadline, "worker completion timed out");
            changed |= collect(controller, pool);
            std::thread::yield_now();
        }
        assert!(changed);
        assert!(!collect(controller, pool));
    }

    #[test]
    fn worker_panics_are_visible_in_the_requesting_model() {
        for kind in [
            RequestKind::Listing,
            RequestKind::Metadata,
            RequestKind::Totals,
        ] {
            let root = tempfile::tempdir().unwrap();
            let mut controller = counting_controller(root.path());
            let metadata = controller.app.request_metadata(0).unwrap();
            let request = match kind {
                RequestKind::Listing => controller.app.request_listing(0, root.path().into()),
                RequestKind::Metadata => metadata,
                RequestKind::Totals => controller.app.request_totals().unwrap(),
            };
            let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
            pool.before_execute = Some(Arc::new(|_, _| panic!("visible panic")));
            pool.submit(request.clone());
            collect_until_idle(&mut controller, &mut pool);
            let error = match kind {
                RequestKind::Listing => controller.app.panes[0].error.as_ref().unwrap(),
                RequestKind::Metadata => controller.app.details[0]
                    .entry
                    .as_ref()
                    .unwrap()
                    .error
                    .as_ref()
                    .unwrap(),
                RequestKind::Totals => controller.app.details[0]
                    .totals
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .unwrap_err(),
            };
            assert!(error.contains("visible panic"));
            assert!(!controller.app.details[0].counting);
            assert!(controller.app.panes[1].error.is_none());
        }
    }

    #[test]
    fn stale_worker_failures_cannot_replace_newer_requests() {
        let root = tempfile::tempdir().unwrap();
        let mut controller = counting_controller(root.path());
        let metadata = controller.app.request_metadata(0).unwrap();
        let totals = controller.app.request_totals().unwrap();
        controller.app.request_totals().unwrap();
        assert!(
            !controller
                .app
                .complete_failure(&totals, "old count panic".into())
        );
        assert!(controller.app.details[0].counting);
        assert!(controller.app.details[0].totals.is_none());
        let listing = controller.app.request_listing(0, root.path().into());
        controller.app.request_listing(0, root.path().into());
        assert!(
            !controller
                .app
                .complete_failure(&listing, "old listing panic".into())
        );
        controller.app.details[0].generation += 1;
        assert!(
            !controller
                .app
                .complete_failure(&metadata, "old metadata panic".into())
        );
        let cached = controller.app.details[0].entry.as_ref().unwrap();
        assert_eq!(cached.path, metadata.path);
        assert!(cached.error.is_none());
        assert!(controller.app.details[0].canonical.is_none());
    }

    #[test]
    fn metadata_debounce_waits_for_the_active_visible_view_and_installs_native_results() {
        let root = tempfile::tempdir().unwrap();
        let mut controller = counting_controller(root.path());
        let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
        let mut pending = [None, None];
        assert!(schedule_metadata(&mut controller, &mut pool, &mut pending));
        let request = pending[0].as_ref().unwrap().0.clone();
        pending[0].as_mut().unwrap().1 = Instant::now() + Duration::from_secs(60);
        controller.app.popup = true;
        schedule_metadata(&mut controller, &mut pool, &mut pending);
        assert_eq!(pool.active_count(), 0);
        pending[0].as_mut().unwrap().1 = Instant::now() - Duration::from_secs(1);
        controller.app.popup = false;
        schedule_metadata(&mut controller, &mut pool, &mut pending);
        assert_eq!(pool.active_count(), 0);
        controller.app.popup = true;
        controller.app.active = 1;
        schedule_metadata(&mut controller, &mut pool, &mut pending);
        assert_eq!(pool.active_count(), 0);
        controller.app.active = 0;
        schedule_metadata(&mut controller, &mut pool, &mut pending);
        assert!(pending[0].is_none());
        assert_eq!(pool.active_count(), 1);
        collect_until_idle(&mut controller, &mut pool);
        let detail = &controller.app.details[0];
        assert_eq!(detail.entry.as_ref().unwrap().path, request.path);
        assert_eq!(
            detail.entry.as_ref().unwrap().kind,
            fs::EntryKind::Directory
        );
        assert_eq!(
            detail.canonical,
            Some(std::fs::canonicalize(&request.path).unwrap())
        );
        assert!(controller.app.details[1].entry.is_none());
        assert!(!controller.preferences_changed);
    }

    #[test]
    fn collected_totals_update_the_requesting_pane_and_report_disappearing_roots() {
        let root = tempfile::tempdir().unwrap();
        let mut controller = counting_controller(root.path());
        let snapshot = fs::list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap();
        controller.app.complete_listing(1, 1, Ok(snapshot));
        let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
        for pane in 0..2 {
            controller.app.active = pane;
            controller.app.request_metadata(pane).unwrap();
            pool.submit(controller.app.request_totals().unwrap());
        }
        controller.app.active = 0;
        collect_until_idle(&mut controller, &mut pool);
        for detail in &controller.app.details {
            let totals = detail.totals.as_ref().unwrap().as_ref().unwrap();
            assert_eq!((totals.files, totals.directories, totals.bytes), (1, 0, 4));
            assert!(!detail.counting);
        }
        let request = controller.app.request_totals().unwrap();
        std::fs::remove_dir_all(&request.path).unwrap();
        pool.submit(request);
        collect_until_idle(&mut controller, &mut pool);
        assert!(controller.app.details[0].totals.as_ref().unwrap().is_err());
        assert!(!controller.app.details[0].counting);
        assert_eq!(
            controller.app.details[1]
                .totals
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap()
                .bytes,
            4
        );
    }

    #[test]
    fn temporary_fuzzy_metadata_requests_clock_redraw_with_exact_listing_defaults() {
        let root = tempfile::tempdir().unwrap();
        let mut c = counting_controller(root.path());
        let g = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 120, 24));
        c.dispatch(Action::Metadata, &g);
        c.app.metadata_formats[1]
            .as_mut()
            .unwrap()
            .toggle(Action::FuzzyTimestamps);
        c.app.now = std::time::SystemTime::UNIX_EPOCH;
        assert!(tick(&mut c.app));
        assert!(c.app.panes.iter().all(|p| !p.fuzzy_timestamps));
        assert!(!c.preferences.fuzzy_timestamps && !c.preferences_changed);
    }

    fn hold_first_count(
        pool: &mut Pool,
    ) -> (mpsc::Receiver<()>, mpsc::SyncSender<()>, mpsc::Receiver<()>) {
        let (entered, running) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::sync_channel(1);
        let (confirmed, stopped) = mpsc::sync_channel(1);
        let wait = Mutex::new(wait);
        let held = AtomicBool::new(false);
        pool.before_execute = Some(Arc::new(move |request, cancelled| {
            if request.kind == RequestKind::Totals
                && !held.swap(true, std::sync::atomic::Ordering::Relaxed)
            {
                entered.send(()).unwrap();
                wait.lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
                assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
                confirmed.send(()).unwrap();
            }
        }));
        (running, release, stopped)
    }

    #[test]
    fn cancel_signals_active_traversal_and_new_count_owns_the_lane() {
        let root = tempfile::tempdir().unwrap();
        let mut controller = counting_controller(root.path());
        let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
        let (running, release, stopped) = hold_first_count(&mut pool);
        let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
        let first = controller.dispatch(Action::Count, &geometry).pop().unwrap();
        pool.submit(first.clone());
        running.recv_timeout(Duration::from_secs(10)).unwrap();
        controller.dispatch(Action::Cancel, &geometry);
        schedule_metadata(&mut controller, &mut pool, &mut [None, None]);
        release.send(()).unwrap();
        // Confirm Esc itself cancelled the worker before submit can also cancel it.
        stopped.recv_timeout(Duration::from_secs(10)).unwrap();
        let next = controller.dispatch(Action::Count, &geometry).pop().unwrap();
        pool.submit(next.clone());
        let old = pool.next(Duration::from_secs(10)).unwrap();
        assert_eq!(old.request.generation, first.generation);
        let Value::Totals(result) = old.value else {
            panic!("totals completion")
        };
        assert_eq!(result.as_ref().unwrap_err(), "inspection cancelled");
        assert!(!controller.app.complete_totals(&old.request, result));
        let fresh = pool.next(Duration::from_secs(10)).unwrap();
        let Value::Totals(result) = fresh.value else {
            panic!("totals completion")
        };
        assert_eq!(result.as_ref().unwrap().bytes, 4);
        assert!(controller.app.complete_totals(&fresh.request, result));
        assert!(
            !controller
                .app
                .complete_totals(&first, Ok(fs::Totals::default()))
        );
        assert_eq!(
            controller.app.details[0]
                .totals
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap()
                .bytes,
            4
        );
        assert_eq!(pool.active_count(), 0);
    }
}
