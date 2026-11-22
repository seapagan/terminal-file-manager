//! Read-only row emphasis, independent of the consumer's interaction state.
use crate::{app::Pane, color::Theme, fs::Entry};
use ratatui::{Frame, layout::Rect};

pub(super) fn dim(
    frame: &mut Frame,
    pane: &Pane,
    area: Rect,
    theme: &Theme,
    preserve: Option<&dyn Fn(&Entry) -> bool>,
) {
    let Some(preserve) = preserve else {
        return;
    };
    let height = usize::from(area.height.saturating_sub(1));
    let start = pane.viewport_start(height);
    for (offset, row) in (start..pane.row_count()).take(height).enumerate() {
        let entry = row
            .checked_sub(usize::from(pane.has_parent()))
            .and_then(|row| pane.visible.get(row))
            .map(|i| &pane.entries[*i]);
        if entry.is_some_and(|entry| entry.error.is_some() || preserve(entry)) {
            continue;
        }
        for x in area.x..area.right() {
            let cell = &mut frame.buffer_mut()[(x, area.y + 1 + offset as u16)];
            cell.set_style(theme.dim_listing_style(cell.style()));
        }
    }
}
