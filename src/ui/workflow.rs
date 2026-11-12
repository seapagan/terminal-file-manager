//! Generic one-row workflow presentation. Rendering and hit testing share measurements.
use crate::{
    color::{Role, Theme},
    workflow::{Flow, Target},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};

#[derive(Clone, Debug)]
pub struct Control {
    pub area: Rect,
    pub label: String,
    pub target: Option<Target>,
    pub enabled: bool,
    pub selected: bool,
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub controls: Vec<Control>,
}

impl Layout {
    pub fn hit(&self, column: u16, row: u16) -> Option<Target> {
        self.controls
            .iter()
            .find(|c| c.area.contains((column, row).into()))
            .and_then(|c| c.target)
    }
}

/// Whole controls only: elided labels never leave invisible mouse targets.
pub fn layout<C, O>(flow: &Flow<C, O>, area: Rect) -> Layout {
    let mut layout = Layout {
        controls: Vec::new(),
    };
    if area.height == 0 || area.width == 0 {
        return layout;
    }
    let tier = if area.width >= 100 {
        0
    } else if area.width >= 65 {
        1
    } else {
        2
    };
    let title = if tier == 2 {
        format!("{}:{}", flow.label, flow.stage_index() + 1)
    } else {
        format!("{} / {}", flow.label, flow.stage().label)
    };
    let navigation_width = if flow.can_back() && area.width >= 13 {
        13
    } else {
        3
    };
    let optional = Rect {
        width: area.width.saturating_sub(navigation_width + 1),
        ..area
    };
    let mut x = area.x;
    append(&mut layout, &mut x, optional, title, None, true, false);
    choices(&mut layout, &mut x, optional, flow, tier);
    navigation(&mut layout, &mut x, area, flow.can_back());
    layout
}

fn navigation(layout: &mut Layout, x: &mut u16, area: Rect, can_back: bool) {
    if can_back && area.width >= 13 {
        append(
            layout,
            x,
            area,
            "Backspace".into(),
            Some(Target::Back),
            true,
            false,
        );
    }
    append(
        layout,
        x,
        area,
        "Esc".into(),
        Some(Target::Cancel),
        true,
        false,
    );
}

fn choices<C, O>(layout: &mut Layout, x: &mut u16, area: Rect, flow: &Flow<C, O>, tier: usize) {
    let compact = flow
        .stage()
        .choices
        .iter()
        .map(|choice| {
            super::modal::width(&format!("{} {}", choice.key_label(), choice.labels[tier])) + 3
        })
        .map(u32::from)
        .sum::<u32>()
        > u32::from(area.right().saturating_sub(*x));
    for choice in &flow.stage().choices {
        let selected = flow.highlighted().is_some_and(|c| c.id == choice.id);
        let text = if compact {
            choice.key_label()
        } else {
            format!(
                "{} {}",
                choice.key_label(),
                crate::ui::clean(&choice.labels[tier])
            )
        };
        let label = format!("[{text}]");
        append(
            layout,
            x,
            area,
            label,
            Some(Target::Choice(choice.id)),
            choice.disabled.is_none(),
            selected,
        );
    }
}

fn append(
    layout: &mut Layout,
    x: &mut u16,
    area: Rect,
    label: String,
    target: Option<Target>,
    enabled: bool,
    selected: bool,
) {
    let width = super::modal::width(&label);
    if width > area.right().saturating_sub(*x) {
        return;
    }
    layout.controls.push(Control {
        area: Rect::new(*x, area.y, width, 1),
        label,
        target,
        enabled,
        selected,
    });
    *x = x.saturating_add(width).saturating_add(1);
}

pub fn render(frame: &mut Frame, layout: &Layout, theme: &Theme) {
    for control in &layout.controls {
        let role = if !control.enabled {
            Role::Disabled
        } else if control.selected {
            Role::Key
        } else {
            Role::Value
        };
        let mut style = theme.style(role);
        if control.selected {
            style = style.add_modifier(Modifier::BOLD | Modifier::REVERSED);
        }
        frame.render_widget(Paragraph::new(content(control, style, theme)), control.area);
    }
}

fn content<'a>(control: &'a Control, style: ratatui::style::Style, theme: &Theme) -> Line<'a> {
    let Some(text) = control
        .label
        .strip_prefix('[')
        .and_then(|label| label.strip_suffix(']'))
    else {
        return Line::from(Span::styled(&control.label, style));
    };
    let brackets = if control.selected || !control.enabled {
        style
    } else {
        theme.style(Role::Muted)
    };
    Line::from(vec![
        Span::styled("[", brackets),
        Span::styled(text, style),
        Span::styled("]", brackets),
    ])
}
