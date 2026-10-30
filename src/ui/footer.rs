use crate::{
    color::{Role, Theme},
    input::Action,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

/// Visible footer labels and their exact hit areas share one geometry contract.
#[derive(Clone, Debug)]
pub struct FooterControl {
    pub area: Rect,
    pub label: &'static str,
    pub action: Action,
}

pub(super) fn controls(area: Rect) -> Vec<FooterControl> {
    let mut controls = Vec::new();
    let mut x = area.x;
    for (label, action) in [
        ("? Help", Action::Help),
        ("F5 Copy", Action::Copy),
        ("F6 Move", Action::Move),
        ("F7 Mkdir", Action::Mkdir),
        ("Alt+N New file", Action::CreateFile),
        ("F8 Trash", Action::Trash),
        ("F9 Jobs", Action::Jobs),
        ("Alt+Enter Info", Action::Metadata),
        ("q Quit", Action::Quit),
        ("Alt+B Mark", Action::PatternSelection),
        ("Alt+P Panes", Action::TogglePanes),
        ("V View", Action::ViewOptions),
        ("/ Filter", Action::Filter),
        ("Alt+V Profiles", Action::ViewProfiles),
    ] {
        let width = label.len() as u16;
        if u32::from(x) + u32::from(width) > u32::from(area.right()) {
            break;
        }
        controls.push(FooterControl {
            area: Rect::new(x, area.y, width, area.height),
            label,
            action,
        });
        x = x.saturating_add(width + 2);
    }
    controls
}

pub(super) fn render(frame: &mut Frame, area: Rect, controls: &[FooterControl], theme: &Theme) {
    let mut spans = Vec::new();
    for control in controls {
        let (key, label) = control.label.split_once(' ').unwrap();
        spans.push(Span::styled(key, theme.style(Role::Key)));
        spans.push(Span::styled(
            format!(" {label}  "),
            theme.style(Role::Value),
        ));
    }
    spans.push(Span::styled(
        "F3/F4 viewer/editor unavailable",
        theme.style(Role::Disabled),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
