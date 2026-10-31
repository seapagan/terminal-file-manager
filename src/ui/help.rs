//! One measured document supplies rendering, scroll bounds and jump anchors.
pub(super) mod context;
mod picker;
use crate::{
    app::App,
    color::{Role, Theme},
    input::{HelpSection, HelpTarget},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

#[derive(Clone, Debug)]
struct Row {
    keys: String,
    text: String,
    heading: bool,
    target: HelpTarget,
}

/// Rendered document and temporary picker geometry, shared with input hit testing.
#[derive(Clone, Debug)]
pub struct HelpLayout {
    pub outer: Rect,
    pub body: Rect,
    pub search: Option<Rect>,
    pub anchors: Vec<(HelpTarget, u16)>,
    pub scroll_limit: u16,
    pub picker: Option<picker::PickerLayout>,
    rows: Vec<Row>,
    keys_width: usize,
}

pub(super) fn layout(app: &App, bounds: Rect) -> Option<HelpLayout> {
    if !app.help {
        return None;
    }
    let width = bounds.width.saturating_sub(2).min(110);
    let keys_width = app
        .help_rows
        .iter()
        .map(|entry| Line::raw(&entry.keys).width())
        .max()
        .unwrap_or(0)
        .min(usize::from(width.saturating_sub(3)) / 3);
    let (rows, anchors) = document(app, usize::from(width.saturating_sub(2)), keys_width);
    let height = (rows.len() + 2).min(usize::from(bounds.height.saturating_sub(2))) as u16;
    let outer = super::centered(bounds, width, height);
    let body = Rect::new(
        outer.x.saturating_add(1),
        outer.y.saturating_add(1),
        width.saturating_sub(2),
        height.saturating_sub(2),
    );
    let search = (width >= 33 && height > 0).then(|| Rect::new(outer.right() - 10, outer.y, 8, 1));
    let scroll_limit = rows
        .len()
        .saturating_sub(usize::from(body.height))
        .min(usize::from(u16::MAX)) as u16;
    Some(HelpLayout {
        outer,
        body,
        search,
        anchors,
        scroll_limit,
        picker: picker::layout(app, bounds),
        rows,
        keys_width,
    })
}

fn document(app: &App, width: usize, keys_width: usize) -> (Vec<Row>, Vec<(HelpTarget, u16)>) {
    let mut rows = Vec::new();
    let mut anchors = Vec::new();
    for section in HelpSection::ALL {
        let target = HelpTarget::Section(section);
        if !rows.is_empty() {
            rows.push(Row {
                keys: String::new(),
                text: String::new(),
                heading: false,
                target,
            });
        }
        anchors.push((target, rows.len() as u16));
        for text in super::text::wrap_value(section.title(), width) {
            rows.push(Row {
                keys: String::new(),
                text,
                heading: true,
                target,
            });
        }
        for entry in app
            .help_rows
            .iter()
            .filter(|entry| entry.action.help_section() == section)
        {
            let target = HelpTarget::Action(entry.action);
            anchors.push((target, rows.len() as u16));
            let keys = super::text::wrap_value(&entry.keys, keys_width);
            let text =
                super::text::wrap_value(entry.description, width.saturating_sub(keys_width + 1));
            for index in 0..keys.len().max(text.len()) {
                rows.push(Row {
                    keys: keys.get(index).cloned().unwrap_or_default(),
                    text: text.get(index).cloned().unwrap_or_default(),
                    heading: false,
                    target,
                });
            }
        }
    }
    (rows, anchors)
}

pub(super) fn render(frame: &mut Frame, app: &App, theme: &Theme, layout: &HelpLayout) {
    super::modal::render_shell(frame, layout.outer, "Help — Esc closes", theme);
    if let Some(area) = layout.search {
        frame.render_widget(
            Paragraph::new("[/ Jump]").style(theme.style(Role::Key)),
            area,
        );
    }
    let lines: Vec<_> = layout
        .rows
        .iter()
        .map(|row| line(row, app, theme, layout.keys_width))
        .collect();
    frame.render_widget(
        Paragraph::new(lines).scroll((app.overlay_scroll.min(layout.scroll_limit), 0)),
        layout.body,
    );
    if let Some(picker) = &layout.picker {
        picker::render(frame, app, theme, picker);
    }
}

fn line(row: &Row, app: &App, theme: &Theme, key_width: usize) -> Line<'static> {
    let mut line = if row.heading {
        Line::styled(row.text.clone(), theme.style(Role::SectionHeader))
    } else {
        let padding = key_width.saturating_sub(Line::raw(&row.keys).width()) + 1;
        Line::from(vec![
            Span::styled(
                format!("{}{}", row.keys, " ".repeat(padding)),
                theme.style(Role::Key),
            ),
            Span::styled(row.text.clone(), theme.style(Role::Value)),
        ])
    };
    if app
        .help_landing
        .is_some_and(|(target, until)| target == row.target && app.now < until)
    {
        line = line.style(theme.style(Role::Cursor));
    }
    line
}
