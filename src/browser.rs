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
use colored_text::{ColorizeConfig, RenderTarget};
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
    let theme = Theme::new(ColorizeConfig::color_level(RenderTarget::Stdout));
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
