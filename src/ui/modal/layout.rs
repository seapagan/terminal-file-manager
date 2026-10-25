use super::{Spec, actions::Action, width};
use ratatui::layout::Rect;

/// Exact visible action rectangle, including its brackets.
#[derive(Clone, Debug)]
pub struct ActionRegion<T> {
    /// Original action index, retained when other controls cannot fit.
    pub index: usize,
    /// Rendered and interactive bounds.
    pub area: Rect,
    /// Action shown at these bounds.
    pub action: Action<T>,
}

/// Immutable measured geometry. The body alone scrolls; title/actions remain fixed.
#[derive(Clone, Debug)]
pub struct Layout<T> {
    /// Bordered shell.
    pub outer: Rect,
    /// Fixed leading region.
    pub pinned: Rect,
    /// Scrollable body, excluding padding and controls.
    pub body: Rect,
    /// Exact action hit regions.
    pub actions: Vec<ActionRegion<T>>,
}

impl<T: Clone> Layout<T> {
    pub(super) fn new(spec: &Spec<'_, T>, terminal: Rect, rows: impl FnOnce(u16) -> usize) -> Self {
        let available = Rect {
            height: terminal.height.saturating_sub(2),
            ..terminal
        };
        let outer_width = measured_width(spec, terminal.width);
        let padding = if outer_width >= 4 {
            2
        } else {
            outer_width.min(1)
        };
        let inner_width = outer_width.saturating_sub(padding * 2);
        let mut actions = pack(spec.actions, inner_width);
        let action_rows = actions.last().map_or(0, |a| a.area.y + 1);
        let body_rows = rows(inner_width).min(usize::from(u16::MAX)) as u16;
        let desired_height = body_rows
            .saturating_add(spec.pinned_rows)
            .saturating_add(action_rows)
            .saturating_add(4 + u16::from(action_rows > 0));
        let outer = super::super::centered(available, outer_width, desired_height);
        let inset = outer.height.min(if outer.height >= 7 { 2 } else { 1 });
        let top = outer.y.saturating_add(inset);
        let bottom = outer
            .bottom()
            .saturating_sub(inset)
            .max(top.min(outer.bottom()));
        let action_top = bottom.saturating_sub(action_rows).max(top);
        let pinned_height = spec.pinned_rows.min(action_top.saturating_sub(top));
        let pinned = Rect::new(outer.x + padding, top, inner_width, pinned_height);
        let body = Rect::new(
            pinned.x,
            pinned.bottom(),
            inner_width,
            action_top
                .saturating_sub(pinned.bottom())
                .saturating_sub(u16::from(action_rows > 0)),
        );
        position(&mut actions, pinned.x, action_top);
        actions.retain(|a| a.area.bottom() <= bottom && a.area.right() <= outer.right());
        Self {
            outer,
            pinned,
            body,
            actions,
        }
    }

    /// Maximum body offset, independent of the number of retained visible rows.
    pub fn scroll_limit(&self, rows: usize) -> usize {
        rows.saturating_sub(usize::from(self.body.height))
    }

    /// Disabled controls never produce an intent.
    pub fn hit(&self, column: u16, row: u16) -> Option<&T> {
        self.actions
            .iter()
            .find(|a| a.action.enabled && a.area.contains((column, row).into()))
            .map(|a| &a.action.intent)
    }
}

fn pack<T: Clone>(actions: &[Action<T>], available: u16) -> Vec<ActionRegion<T>> {
    let mut regions = Vec::new();
    let (mut x, mut y) = (0u16, 0u16);
    for (index, action) in actions.iter().enumerate() {
        let length = width(action.label).saturating_add(2);
        if length > available {
            continue;
        }
        if x.saturating_add(length) > available {
            x = 0;
            y += 1;
        }
        regions.push(ActionRegion {
            index,
            area: Rect::new(x, y, length, 1),
            action: action.clone(),
        });
        x = x.saturating_add(length).saturating_add(1);
    }
    for row in 0..=y {
        let used = regions
            .iter()
            .filter(|a| a.area.y == row)
            .map(|a| a.area.right())
            .max()
            .unwrap_or(0);
        for region in regions.iter_mut().filter(|a| a.area.y == row) {
            region.area.x += available.saturating_sub(used) / 2;
        }
    }
    regions
}

fn position<T>(actions: &mut [ActionRegion<T>], left: u16, top: u16) {
    for action in actions {
        action.area.x += left;
        action.area.y += top;
    }
}

fn measured_width<T>(spec: &Spec<'_, T>, terminal_width: u16) -> u16 {
    let action_width = spec
        .actions
        .iter()
        .map(|a| usize::from(width(a.label)) + 3)
        .sum::<usize>()
        .saturating_sub(1);
    let desired = spec
        .content_width
        .max(action_width)
        .saturating_add(4)
        .max(usize::from(width(spec.title)).saturating_add(2));
    let max_width = terminal_width.saturating_sub(u16::from(terminal_width > 4) * 2);
    desired.min(usize::from(max_width)) as u16
}
