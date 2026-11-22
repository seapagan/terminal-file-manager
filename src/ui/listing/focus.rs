//! Read-only browser emphasis, independent of the consumer's interaction state.
use crate::{app::Pane, color::Theme, fs::Entry};
use ratatui::{Frame, layout::Rect};

pub(super) fn dim(
    frame: &mut Frame,
    pane: &Pane,
    area: Rect,
    rows: Rect,
    theme: &Theme,
    preserve: Option<&dyn Fn(&Entry) -> bool>,
) {
    let Some(preserve) = preserve else {
        return;
    };
    let height = usize::from(rows.height.saturating_sub(1));
    let start = pane.viewport_start(height);
    for y in area.y..area.bottom() {
        let bright = y
            .checked_sub(rows.y.saturating_add(1))
            .filter(|offset| usize::from(*offset) < height)
            .and_then(|offset| {
                (start + usize::from(offset)).checked_sub(usize::from(pane.has_parent()))
            })
            .and_then(|row| pane.visible.get(row))
            .is_some_and(|i| preserve(&pane.entries[*i]));
        for x in area.x..area.right() {
            if bright && (rows.x..rows.right()).contains(&x) {
                continue;
            }
            mute_cell(frame, x, y, theme);
        }
    }
}

pub(in crate::ui) fn surfaces(frame: &mut Frame, geometry: &crate::ui::Geometry, theme: &Theme) {
    for area in [Some(geometry.status), geometry.sidebar, geometry.divider]
        .into_iter()
        .flatten()
    {
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                mute_cell(frame, x, y, theme);
            }
        }
    }
}

fn mute_cell(frame: &mut Frame, x: u16, y: u16, theme: &Theme) {
    let cell = &mut frame.buffer_mut()[(x, y)];
    cell.set_style(theme.dim_listing_style(cell.style()));
}
