//! Apply read-only worker completions and avoid redraws for unchanged listings.
use super::{App, Controller, Duration, Pool, Value};

pub(super) fn tick(app: &mut App) -> bool {
    let now = std::time::SystemTime::now();
    let elapsed = now.duration_since(app.now).unwrap_or(Duration::MAX);
    if elapsed < Duration::from_secs(1) {
        return false;
    }
    app.now = now;
    let help_expired = app.help_landing.is_some_and(|(_, until)| now >= until);
    if help_expired {
        app.help_landing = None;
    }
    help_expired
        || app.notifications.open
        || app.operations.active()
        || app.panes.iter().any(|pane| pane.fuzzy_timestamps)
        || ((app.popup || app.sidebar) && app.metadata_format(true).fuzzy_timestamps)
}

pub(super) fn collect(controller: &mut Controller, pool: &mut Pool) -> bool {
    let mut changed = false;
    for _ in 0..6 {
        let Some(completion) = pool.next(Duration::ZERO) else {
            break;
        };
        let request = completion.request;
        match completion.value {
            Value::Listing(result) => {
                let pane = request.pane;
                let before = listing_presentation(&controller.app, pane);
                controller
                    .app
                    .complete_listing(pane, request.generation, result);
                changed |= before != listing_presentation(&controller.app, pane);
            }
            Value::Metadata(entry, canonical) => {
                controller
                    .app
                    .complete_metadata(&request, *entry, canonical);
                changed = true;
            }
            Value::Totals(result) => {
                controller.app.complete_totals(&request, result);
                changed = true;
            }
            Value::Failed(error) => {
                controller.app.complete_failure(&request, error);
                changed = true;
            }
        }
    }
    changed
}

fn listing_presentation(
    app: &App,
    pane: usize,
) -> (u64, Option<String>, Option<std::time::SystemTime>) {
    (
        app.panes[pane].listing_installs,
        app.panes[pane].error.clone(),
        app.details[pane].entry.as_ref().and_then(|e| e.accessed),
    )
}
