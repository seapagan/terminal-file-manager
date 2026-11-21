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

pub(super) fn layout(app: &App, area: Rect, controls: &[PolicyControl]) -> Option<PolicyLayout> {
    let menu = app
        .policy_menu
        .as_ref()
        .filter(|_| !policy_menu::preempted(app))?;
    let anchor = controls
        .iter()
        .find(|c| c.pane == menu.pane && c.action == menu.action)?
        .area;
    let choices = policy_menu::choices(menu.action);
    let content_width = choice_width(choices);
    let width = content_width.saturating_add(4).min(area.width);
    let available = anchor.y.saturating_sub(area.y);
    let height = (choices.len() as u16 + 2).min(if available >= 3 {
        available
    } else {
        area.height
    });
    let x = anchor.x.min(area.right().saturating_sub(width)).max(area.x);
    let y = if available >= 3 {
        anchor.y.saturating_sub(height)
    } else {
        area.y
    };
    let spec = super::modal::Spec {
        title: "",
        help: false,
        content_width: usize::from(content_width),
        pinned_rows: 0,
        footer_rows: 0,
        actions: &[],
    };
    let mut shell = super::modal::Layout::viewport(&spec, Rect::new(x, y, width, height));
    shell.close = None;
    // Choice rows sit directly inside the border, without form padding.
    shell.body.y = y.saturating_add(u16::from(height > 0));
    shell.body.height = height.saturating_sub(2);
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
