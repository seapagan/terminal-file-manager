use super::{OperationControl, OperationTarget, actions, content};
use crate::app::App;
use ratatui::{layout::Rect, text::Line};

#[derive(Clone, Debug)]
pub(in crate::ui) struct Layout {
    pub outer: Rect,
    pub body: Rect,
    pub list: Rect,
    pub buttons: Vec<OperationControl>,
    pub(super) rows: Vec<content::Row>,
}

impl Layout {
    pub fn new(app: &App, terminal: Rect) -> Self {
        let rows = content::rows(app);
        let width = desired_width(app, &rows).min(
            terminal
                .width
                .saturating_sub(u16::from(terminal.width > 4) * 2),
        );
        let padding = if width >= 8 { 2 } else { 1 };
        let inner = width.saturating_sub(padding * 2);
        let buttons = pack(app, inner);
        let button_rows = buttons.last().map_or(0, |button| button.area.y + 1);
        let rows = content::wrapped(rows, inner);
        let list_rows = list_rows(app, terminal.height);
        let height = ((rows.len().min(u16::MAX as usize) as u16)
            .saturating_add(list_rows)
            .saturating_add(button_rows)
            .saturating_add(4))
        .min(terminal.height.saturating_sub(2));
        let available = Rect::new(
            terminal.x,
            terminal.y,
            terminal.width,
            terminal.height.saturating_sub(2),
        );
        let outer = super::super::centered(available, width, height);
        let list = Rect::new(
            outer.x + padding.min(width),
            outer.y.saturating_add(1),
            inner,
            list_rows.min(height.saturating_sub(2)),
        );
        let content_top = list.bottom();
        let button_top = outer
            .bottom()
            .saturating_sub(button_rows.saturating_add(1))
            .max(content_top);
        let body = Rect::new(
            list.x,
            content_top,
            inner,
            button_top.saturating_sub(content_top).saturating_sub(1),
        );
        Self {
            outer,
            body,
            list,
            buttons: position(buttons, list, button_top, outer),
            rows,
        }
    }

    pub fn limit(&self, rows: usize) -> usize {
        rows.saturating_sub(usize::from(self.body.height))
    }
}

fn desired_width(app: &App, rows: &[content::Row]) -> u16 {
    let mut longest = Line::raw(content::title(app)).width();
    for row in rows {
        longest = longest.max(Line::raw(super::super::clean(&row.text)).width());
        if longest >= 84 {
            return 88;
        }
    }
    longest.saturating_add(4).clamp(38, 88) as u16
}

fn pack(app: &App, width: u16) -> Vec<OperationControl> {
    let mut controls = Vec::new();
    let mut x: u16 = 0;
    let mut y: u16 = 0;
    for (label, key) in actions::buttons(app, width < 40) {
        let length = label.len() as u16 + 2;
        if length > width {
            continue;
        }
        if x.saturating_add(length) > width {
            x = 0;
            y = y.saturating_add(1);
        }
        controls.push(OperationControl {
            area: Rect::new(x, y, length, 1),
            target: OperationTarget::Key(key),
            label,
        });
        x += length + 1;
    }
    controls
}

fn position(
    buttons: Vec<OperationControl>,
    list: Rect,
    top: u16,
    outer: Rect,
) -> Vec<OperationControl> {
    buttons
        .into_iter()
        .filter_map(|mut button| {
            button.area.x += list.x;
            button.area.y += top;
            (button.area.bottom() < outer.bottom()
                && button.area.right() <= outer.right().saturating_sub(1))
            .then_some(button)
        })
        .collect()
}

fn list_rows(app: &App, height: u16) -> u16 {
    if app.operations.show_history && app.operations.dialog.is_none() {
        app.operations
            .history
            .len()
            .min(5)
            .min(usize::from(height / 4)) as u16
    } else {
        0
    }
}
