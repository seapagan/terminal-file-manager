//! Wrapped full-path selection uses the shared content-sized modal shell.
use super::{PathControl, modal};
use crate::{
    app::App,
    color::{Role, Theme},
    navigation::path_segments,
};
use crossterm::event::KeyCode;
use ratatui::{Frame, layout::Rect, style::Style, text::Line, widgets::Paragraph};

#[derive(Clone, Debug)]
pub struct PathLayout {
    pub shell: modal::Layout<()>,
    pub controls: Vec<(PathControl, usize)>,
    pub segment_rows: Vec<usize>,
    pub scroll: usize,
    pub limit: usize,
}

type Wrapped = Vec<Vec<(String, usize)>>;
const HINT: &str = "Arrows select · Enter open";

pub(super) fn layout(app: &App, area: Rect) -> Option<PathLayout> {
    let state = app.path_picker.as_ref()?;
    let segments = path_segments(&state.path);
    let actions = [modal::actions::Action::custom(
        "Esc Cancel",
        (),
        KeyCode::Esc,
    )];
    let shell = modal::Spec {
        title: "Go to directory",
        help: false,
        content_width: segments
            .iter()
            .map(|segment| Line::raw(&segment.text).width())
            .sum::<usize>()
            .max(Line::raw(HINT).width()),
        pinned_rows: 0,
        footer_rows: 1,
        actions: &actions,
    }
    .measure(area, |width| wrap(&segments, width).len());
    let rows = wrap(&segments, shell.body.width);
    let limit = shell.scroll_limit(rows.len());
    let scroll = state.scroll.min(limit);
    let segment_rows = (0..segments.len())
        .map(|index| {
            rows.iter()
                .position(|row| row.iter().any(|(_, i)| *i == index))
                .unwrap_or(0)
        })
        .collect();
    let controls = controls(&rows, &shell, scroll, state, &segments);
    Some(PathLayout {
        shell,
        controls,
        segment_rows,
        scroll,
        limit,
    })
}

fn controls(
    rows: &Wrapped,
    shell: &modal::Layout<()>,
    scroll: usize,
    state: &crate::navigation::PathPicker,
    segments: &[crate::navigation::PathSegment],
) -> Vec<(PathControl, usize)> {
    let mut controls = Vec::new();
    for (y, row) in rows
        .iter()
        .skip(scroll)
        .take(usize::from(shell.body.height))
        .enumerate()
    {
        let mut x = shell.body.x;
        for (text, index) in row {
            let width = modal::width(text);
            controls.push((
                PathControl {
                    area: Rect::new(x, shell.body.y + y as u16, width, 1),
                    pane: state.pane,
                    source: state.path.clone(),
                    target: Some(segments[*index].path.clone()),
                    text: text.clone(),
                },
                *index,
            ));
            x += width;
        }
    }
    controls
}

fn wrap(segments: &[crate::navigation::PathSegment], width: u16) -> Wrapped {
    if width < 2 {
        return Vec::new();
    }
    let mut rows: Wrapped = vec![Vec::new()];
    let mut used = 0;
    for (index, segment) in segments.iter().enumerate() {
        let length = modal::width(&segment.text);
        if length <= width && used > 0 && used + length > width {
            rows.push(Vec::new());
            used = 0;
        }
        let line = Line::raw(&segment.text);
        for grapheme in line.styled_graphemes(Style::default()) {
            let length = modal::width(grapheme.symbol);
            if used + length > width {
                rows.push(Vec::new());
                used = 0;
            }
            let row = rows.last_mut().unwrap();
            if let Some((text, _)) = row.last_mut().filter(|(_, i)| *i == index) {
                text.push_str(grapheme.symbol);
            } else {
                row.push((grapheme.symbol.to_owned(), index));
            }
            used += length;
        }
    }
    rows
}

pub(super) fn render(frame: &mut Frame, app: &App, theme: &Theme, layout: Option<&PathLayout>) {
    let Some(layout) = layout else { return };
    let state = app.path_picker.as_ref().unwrap();
    modal::render_measured(frame, &layout.shell, "Go to directory", theme, Role::Header);
    let hovered = app.path_pointer.and_then(|(x, y)| {
        layout
            .controls
            .iter()
            .find(|(control, _)| control.area.contains((x, y).into()))
            .map(|(_, index)| *index)
    });
    for (control, index) in &layout.controls {
        let style = if hovered == Some(*index) || *index == state.selected {
            theme.style(Role::PathHover)
        } else {
            theme
                .style(Role::Header)
                .remove_modifier(ratatui::style::Modifier::BOLD)
        };
        frame.render_widget(
            Paragraph::new(control.text.clone()).style(style),
            control.area,
        );
    }
    if layout.limit > 0 && layout.shell.cue.height > 0 {
        frame.render_widget(
            Paragraph::new(format!("{} / {}", layout.scroll + 1, layout.limit + 1))
                .style(theme.style(Role::Muted)),
            layout.shell.cue,
        );
    }
    frame.render_widget(
        Paragraph::new(modal::truncate(HINT, layout.shell.footer.width))
            .style(theme.style(Role::Muted)),
        layout.shell.footer,
    );
    modal::actions::render(frame, &layout.shell, theme, None);
}
