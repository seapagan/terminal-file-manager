//! Apply read-only worker completions and avoid redraws for unchanged listings.
use super::{App, Controller, Duration, NativeTerminal, Pool, Theme, Value, io, ui};

#[cfg(test)]
mod tests;

pub(super) fn tick(app: &mut App, width: u16) -> bool {
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
        || ((app.popup || app.sidebar_visible(width)) && app.metadata_format(true).fuzzy_timestamps)
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
            Value::ParentIdentity(identity) => {
                changed |= controller.app.complete_parent_identity(&request, identity);
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

#[derive(Debug, PartialEq)]
struct ListingPresentation {
    installs: u64,
    error: Option<String>,
    accessed: Option<std::time::SystemTime>,
    pattern_listing: bool,
    cursor: Option<std::path::PathBuf>,
}

fn listing_presentation(app: &App, pane: usize) -> Option<ListingPresentation> {
    let state = app.panes.get(pane)?;
    let detail = app.details.get(pane)?;
    Some(ListingPresentation {
        installs: state.listing_installs,
        error: state.error.clone(),
        accessed: detail.entry.as_ref().and_then(|e| e.accessed),
        pattern_listing: app.pattern.as_ref().is_some_and(|state| state.pane == pane)
            && state.listing,
        cursor: state.cursor.clone(),
    })
}

/// Geometry and cursor shape come from the same completed frame.
pub(super) struct Renderer {
    pub geometry: ui::Geometry,
    shape: Option<bool>,
}
impl Renderer {
    pub fn new(geometry: ui::Geometry) -> Self {
        Self {
            geometry,
            shape: None,
        }
    }
    pub fn draw(
        &mut self,
        terminal: &mut NativeTerminal,
        app: &mut App,
        theme: &Theme,
    ) -> io::Result<()> {
        ui::prepare_notifications(app, terminal.size()?.into());
        terminal.draw(|frame| {
            self.geometry = ui::render(frame, app, theme);
        })?;
        let shape = ui::editing_cursor(app, &self.geometry).map(|c| c.overwrite);
        if self.shape != shape {
            crate::terminal::cursor_style(terminal, shape)?;
            self.shape = shape;
        }
        Ok(())
    }
}
