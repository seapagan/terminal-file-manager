//! Upward-anchored choice menus reuse the measured modal shell.
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
    let menu = app.policy_menu.as_ref()?;
    let anchor = controls
        .iter()
        .find(|c| c.pane == menu.pane && c.action == menu.action)?
        .area;
    let choices = policy_menu::choices(menu.action);
    let width = 24.min(area.width);
    let available = anchor.y.saturating_sub(area.y);
    let height = (choices.len() as u16 + 4).min(if available >= 5 {
        available
    } else {
        area.height
    });
    let x = anchor.x.min(area.right().saturating_sub(width)).max(area.x);
    let y = if available >= 5 {
        anchor.y.saturating_sub(height)
    } else {
        area.y
    };
    let spec = super::modal::Spec {
        title: "Choose — Esc cancels",
        help: false,
        content_width: 20,
        pinned_rows: 0,
        footer_rows: 0,
        actions: &[],
    };
    let shell = super::modal::Layout::viewport(&spec, Rect::new(x, y, width, height));
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
                format!("{} {label}", if index == active { "[x]" } else { "[ ]" }),
                style.add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    super::modal::render_measured(
        frame,
        &layout.shell,
        "Choose — Esc cancels",
        theme,
        Role::Header,
    );
    frame.render_widget(Paragraph::new(rows), layout.shell.body);
}
