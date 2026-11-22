//! Borrow the captured selection; presentation never owns another selection set.
use super::{App, Entry, same_entry};

pub(crate) fn listing_focus(app: &App) -> Option<impl Fn(usize, &Entry) -> bool + '_> {
    let flow = app.footer_workflow.as_ref()?;
    let snapshot = flow
        .context()
        .filter(|snapshot| snapshot.matches_context(app))?;
    let source = flow.resolved().get(1).map(|choice| choice.id);
    Some(move |pane: usize, entry: &Entry| {
        if pane != snapshot.pane {
            return false;
        }
        let marked = || {
            snapshot
                .marked
                .iter()
                .any(|captured| same_entry(captured, entry))
        };
        let highlighted = || {
            snapshot
                .highlighted
                .as_ref()
                .is_some_and(|captured| same_entry(captured, entry))
        };
        match source {
            None => marked() || highlighted(),
            Some("marked") => marked(),
            Some("highlighted") => highlighted(),
            _ => false,
        }
    })
}
