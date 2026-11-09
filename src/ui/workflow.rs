//! Generic one-row workflow presentation. Rendering and hit testing share measurements.
use crate::{
    color::{Role, Theme},
    workflow::{Flow, Target},
};
use ratatui::{Frame, layout::Rect, style::Modifier, text::Span, widgets::Paragraph};

pub struct Control {
    pub area: Rect,
    pub label: String,
    pub target: Option<Target>,
    pub enabled: bool,
    pub selected: bool,
}

pub struct Layout {
    pub controls: Vec<Control>,
}

impl Layout {
    pub fn hit(&self, column: u16, row: u16) -> Option<Target> {
        self.controls
            .iter()
            .find(|c| c.enabled && c.area.contains((column, row).into()))
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
    let mut x = area.x;
    append(&mut layout, &mut x, area, title, None, true, false);
    choices(&mut layout, &mut x, area, flow, tier);
    if flow.stage_index() > 0 {
        append(
            &mut layout,
            &mut x,
            area,
            "< Back".into(),
            Some(Target::Back),
            true,
            false,
        );
    }
    append(
        &mut layout,
        &mut x,
        area,
        "Esc".into(),
        Some(Target::Cancel),
        true,
        false,
    );
    layout
}

fn choices<C, O>(layout: &mut Layout, x: &mut u16, area: Rect, flow: &Flow<C, O>, tier: usize) {
    for choice in &flow.stage().choices {
        let selected = flow.highlighted().is_some_and(|c| c.id == choice.id);
        let text = format!("{} {}", choice.key, crate::ui::clean(&choice.labels[tier]));
        let label = if selected {
            format!("[{text}]")
        } else if choice.disabled.is_some() {
            format!("({text})")
        } else {
            text
        };
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
        frame.render_widget(
            Paragraph::new(Span::styled(&control.label, style)),
            control.area,
        );
    }
}
