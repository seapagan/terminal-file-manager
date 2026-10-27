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
    /// Fixed warning/error region before the action group.
    pub footer: Rect,
    /// Reserved interior gap for a scroll-position cue.
    pub cue: Rect,
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
        let minimum = spec
            .pinned_rows
            .saturating_add(spec.footer_rows)
            .saturating_add(action_rows)
            .saturating_add(5 + u16::from(body_rows > 0));
        let desired_height = body_rows
            .saturating_add(spec.pinned_rows)
            .saturating_add(spec.footer_rows)
            .saturating_add(action_rows)
            .saturating_add(4 + u16::from(action_rows > 0))
            .min(((u32::from(terminal.height) * 3 / 4) as u16).max(minimum));
        let outer = super::super::centered(available, outer_width, desired_height);
        let (pinned, body, footer, bottom, action_top) =
            content_areas(spec, outer, padding, inner_width, action_rows);
        let cue = Rect::new(
            body.x,
            body.bottom(),
            body.width,
            u16::from(body_rows > body.height && body.bottom() < footer.y),
        );
        position(&mut actions, pinned.x, action_top);
        actions.retain(|a| a.area.bottom() <= bottom && a.area.right() <= outer.right());
        Self {
            outer,
            pinned,
            body,
            footer,
            cue,
            actions,
        }
    }

    /// Fixed consumer viewport; content and action eligibility cannot resize it.
    pub fn viewport(spec: &Spec<'_, T>, outer: Rect) -> Self {
        Self::viewport_with_action_rows(spec, outer, 0)
    }

    /// Reserve an action band independently of the currently available controls.
    pub fn viewport_with_action_rows(spec: &Spec<'_, T>, outer: Rect, minimum_rows: u16) -> Self {
        let padding = if outer.width >= 4 {
            2
        } else {
            outer.width.min(1)
        };
        let inner_width = outer.width.saturating_sub(padding * 2);
        let mut actions = pack(spec.actions, inner_width);
        let action_rows = actions.last().map_or(0, |a| a.area.y + 1).max(minimum_rows);
        let (pinned, body, footer, bottom, action_top) =
            content_areas(spec, outer, padding, inner_width, action_rows);
        position(&mut actions, pinned.x, action_top);
        actions.retain(|a| a.area.bottom() <= bottom && a.area.right() <= outer.right());
        Self {
            outer,
            pinned,
            body,
            footer,
            actions,
            cue: Rect::new(
                body.x,
                body.bottom(),
                body.width,
                u16::from(body.bottom() < footer.y),
            ),
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

fn content_areas<T>(
    spec: &Spec<'_, T>,
    outer: Rect,
    padding: u16,
    inner_width: u16,
    action_rows: u16,
) -> (Rect, Rect, Rect, u16, u16) {
    let inset = outer.height.min(if outer.height >= 7 { 2 } else { 1 });
    let top = outer.y.saturating_add(inset);
    let bottom = outer
        .bottom()
        .saturating_sub(inset)
        .max(top.min(outer.bottom()));
    let action_top = bottom.saturating_sub(action_rows).max(top);
    let footer_height = spec.footer_rows.min(action_top.saturating_sub(top));
    let footer_top = action_top.saturating_sub(footer_height);
    let footer = Rect::new(outer.x + padding, footer_top, inner_width, footer_height);
    let pinned_height = spec.pinned_rows.min(footer_top.saturating_sub(top));
    let pinned = Rect::new(outer.x + padding, top, inner_width, pinned_height);
    let body = Rect::new(
        pinned.x,
        pinned.bottom(),
        inner_width,
        footer_top
            .saturating_sub(pinned.bottom())
            .saturating_sub(u16::from(action_rows > 0)),
    );
    (pinned, body, footer, bottom, action_top)
}

fn pack<T: Clone>(actions: &[Action<T>], available: u16) -> Vec<ActionRegion<T>> {
    let mut regions = Vec::new();
    let (mut x, mut y) = (0u16, 0u16);
    for (index, action) in actions.iter().enumerate() {
        let length = width(action.label).max(action.min_width).saturating_add(2);
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
        .map(|a| usize::from(width(a.label).max(a.min_width)) + 3)
        .sum::<usize>()
        .saturating_sub(1);
    let desired = spec
        .content_width
        .max(action_width)
        .saturating_add(4)
        .max(usize::from(width(spec.title)).saturating_add(3));
    let max_width = terminal_width.saturating_sub(u16::from(terminal_width > 4) * 2);
    desired.min(usize::from(max_width)) as u16
}
