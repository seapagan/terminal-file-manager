//! Upward-anchored choice menus reuse the measured modal shell.
#[cfg(test)]
mod tests;
use crate::{
    app::App,
    color::{Role, Theme},
    input::policy_menu,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};

#[derive(Clone, Debug)]
pub struct PolicyControl {
    pub pane: usize,
    pub action: crate::input::Action,
    pub area: Rect,
}
#[derive(Clone, Debug)]
pub struct PolicyLayout {
    pub shell: super::modal::Layout<()>,
    pub start: usize,
}

pub(super) fn layout(
    app: &App,
    area: Rect,
    controls: &[PolicyControl],
    panes: &[Option<Rect>; 2],
) -> Option<PolicyLayout> {
    let menu = app
        .policy_menu
        .as_ref()
        .filter(|_| !policy_menu::preempted(app))?;
    let owner = panes
        .get(menu.pane)
        .copied()
        .flatten()
        .unwrap_or(area)
        .intersection(area);
    let anchor = controls
        .iter()
        .find(|c| c.pane == menu.pane && c.action == menu.action)
        .map_or_else(
            || Rect::new(owner.x, owner.bottom().saturating_sub(2).max(owner.y), 0, 0),
            |c| c.area,
        );
    let outer = popup_bounds(owner, anchor, policy_menu::choices(menu.action));
    let spec = super::modal::Spec {
        title: "",
        help: true,
        // viewport uses the already measured rectangle, not content_width.
        content_width: 0,
        pinned_rows: 0,
        footer_rows: 0,
        actions: &[],
    };
    let mut shell = super::modal::Layout::viewport(&spec, outer);
    shell.close = None;
    // Choice rows sit directly inside the border, without form padding.
    shell.body.y = outer.y.saturating_add(u16::from(outer.height > 0));
    shell.body.height = outer.height.saturating_sub(2);
    shell.pinned = Rect {
        height: 0,
        ..shell.body
    };
    shell.footer = Rect {
        y: shell.body.bottom(),
        ..shell.pinned
    };
    shell.cue = shell.footer;
    let start = menu
        .selected
        .saturating_sub(usize::from(shell.body.height.saturating_sub(1)));
    Some(PolicyLayout { shell, start })
}

fn popup_bounds(owner: Rect, anchor: Rect, choices: &[&str]) -> Rect {
    let width = choice_width(choices).saturating_add(4).min(owner.width);
    let available = anchor.y.saturating_sub(owner.y);
    let height = (choices.len() as u16 + 2).min(if available >= 3 {
        available
    } else {
        owner.height
    });
    let x = anchor
        .x
        .min(owner.right().saturating_sub(width))
        .max(owner.x);
    let y = if available >= 3 {
        anchor.y.saturating_sub(height)
    } else {
        owner.y
    };
    Rect::new(x, y, width, height)
}

pub(super) fn render(frame: &mut Frame, app: &App, layout: &PolicyLayout, theme: &Theme) {
    let menu = app.policy_menu.as_ref().unwrap();
    let active = policy_menu::current(app, menu.pane, menu.action);
    let rows: Vec<_> = policy_menu::choices(menu.action)
        .iter()
        .enumerate()
        .skip(layout.start)
        .take(usize::from(layout.shell.body.height))
        .map(|(index, label)| {
            let style = theme.style(if index == menu.selected {
                Role::Cursor
            } else {
                Role::Value
            });
            Line::from(Span::styled(
                choice_text(label, index == active),
                style.add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    super::modal::render_measured(frame, &layout.shell, "", theme, Role::Header);
    frame.render_widget(Paragraph::new(rows), layout.shell.body);
}

fn choice_text(label: &str, active: bool) -> String {
    format!("{} {label}", if active { "[x]" } else { "[ ]" })
}

fn choice_width(choices: &[&str]) -> u16 {
    choices
        .iter()
        .map(|label| super::modal::width(&choice_text(label, false)))
        .max()
        .unwrap_or_default()
}
