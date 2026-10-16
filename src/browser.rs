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
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Browse an explicit native directory; failures never substitute another target.
///
/// # Errors
/// Reports path/configuration/nonterminal/setup/application/persistence errors.
pub fn run(path: &Path) -> Result<(), String> {
    let path = startup_path(path)?;
    let mut store = crate::settings::config_path()
        .map(Store::load)
        .transpose()?;
    let settings = store
        .as_ref()
        .map_or_else(Settings::default, |store| store.settings.clone());
    let mut app = App::new(path);
    settings.apply(&mut app);
    let bindings = Bindings::new(&settings.bindings)?;
    let mut controller = Controller::new(app, bindings);
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
    if controller.preferences_changed {
        let Some(store) = store.as_mut() else {
            return Err("preferences changed but no user config directory is available".into());
        };
        store.settings.capture(&controller.app);
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

fn startup_path(path: &Path) -> Result<PathBuf, String> {
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    std::fs::read_dir(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("interactive browsing requires a terminal on stdin and stdout".into());
    }
    Ok(path)
}
