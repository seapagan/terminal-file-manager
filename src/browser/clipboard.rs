use crate::{clipboard::runtime::Runtime, input::Controller, notifications::Severity};

pub(super) fn collect(c: &mut Controller, runtime: &mut Runtime) -> bool {
    let Some(result) = runtime.poll(&mut std::io::stdout().lock()) else {
        return false;
    };
    match result {
        Ok(message) => c.app.status = message.into(),
        Err(error) => c.app.notify(Severity::Error, error),
    }
    true
}

pub(super) fn submit(c: &mut Controller, runtime: &mut Runtime) {
    let Some(request) = c.path_copy_request.take() else {
        return;
    };
    let scope = request.scope.clone();
    match runtime.start(request, c.preferences.clipboard.osc52_fallback) {
        Ok(()) => c.app.status = format!("Copying {scope} to clipboard…").into(),
        Err(error) => c.app.status = error.into(),
    }
}
