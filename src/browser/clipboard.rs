use crate::{
    clipboard::runtime::{Completion, Runtime},
    input::Controller,
    notifications::Severity,
};

pub(super) fn collect(c: &mut Controller, runtime: &mut Runtime) -> bool {
    let Some(result) = runtime.poll(&mut std::io::stdout().lock()) else {
        return false;
    };
    let message = result.and_then(|done| match done {
        Completion::Message(message) => Ok(message),
        Completion::Base(request) => request.apply(&mut c.app),
    });
    match message {
        Ok(message) => c.app.status = message.into(),
        Err(error) => c.app.notify(Severity::Error, error),
    }
    true
}

pub(super) fn submit(c: &mut Controller, runtime: &mut Runtime) {
    if let Some(request) = c.path_base_request.take()
        && let Err(error) = runtime.start_base(request)
    {
        c.app.status = error.into();
    }
    let Some(request) = c.path_copy_request.take() else {
        return;
    };
    let scope = request.scope.clone();
    match runtime.start(request, c.preferences.clipboard.osc52_fallback) {
        Ok(()) => c.app.status = format!("Copying {scope} to clipboard…").into(),
        Err(error) => c.app.status = error.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use std::time::{Duration, Instant};

    fn controller(path: &std::path::Path) -> Controller {
        let mut app = crate::app::App::new(path.into());
        let listing = crate::fs::list(path, &Default::default(), &Default::default()).unwrap();
        app.complete_listing(0, 1, Ok(listing));
        Controller::new(
            app,
            crate::input::Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        )
    }

    #[test]
    fn browser_submits_base_work_and_applies_only_a_current_result() {
        let root = tempfile::tempdir().unwrap();
        for stale in [false, true] {
            let mut c = controller(root.path());
            let original = root.path().join("original");
            c.app.relative_path_base = original.clone();
            for key in ['y', 's'] {
                let geometry =
                    crate::ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 120, 24));
                c.handle(
                    Event::Key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE)),
                    &geometry,
                    Default::default(),
                );
            }
            assert_eq!(c.app.relative_path_base, original);
            assert!(c.path_copy_request.is_none());
            let mut runtime = Runtime::default();
            submit(&mut c, &mut runtime);
            assert!(c.path_base_request.is_none());
            if stale {
                c.app.request_listing(0, root.path().join("elsewhere"));
            }
            let deadline = Instant::now() + Duration::from_secs(10);
            while !collect(&mut c, &mut runtime) {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            assert_eq!(
                c.app.relative_path_base,
                if stale { original } else { root.path().into() }
            );
            assert!(!runtime.busy());
            assert!(c.path_copy_request.is_none());
        }
    }
}
