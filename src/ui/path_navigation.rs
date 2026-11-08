//! Measured native path controls shared by drawing and input.
use crate::{
    app::App,
    color::{Role, Theme},
    navigation::path_segments,
};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};
use std::path::PathBuf;

/// One rendered path slice; None targets the full-path picker.
#[derive(Clone, Debug)]
pub struct PathControl {
    pub area: Rect,
    pub pane: usize,
    pub source: PathBuf,
    pub target: Option<PathBuf>,
    pub text: String,
}

pub(super) fn headers(app: &App, panes: &[Option<Rect>; 2]) -> Vec<PathControl> {
    panes
        .iter()
        .enumerate()
        .flat_map(|(pane, area)| area.map(|area| header(app, pane, area)).unwrap_or_default())
        .collect()
}

fn header(app: &App, pane: usize, area: Rect) -> Vec<PathControl> {
    let prefix = format!(
        "{} Pane {}: ",
        if pane == app.active { ">" } else { " " },
        pane + 1
    );
    let loading = if app.panes[pane].pending.is_some() {
        10
    } else {
        0
    };
    let width = area.width.saturating_sub(2 + prefix.len() as u16 + loading);
    if area.height == 0 || width == 0 {
        return Vec::new();
    }
    let mut x = area.x + 1 + prefix.len() as u16;
    pieces(&app.panes[pane].path, width)
        .into_iter()
        .filter_map(|(text, target)| {
            let width = super::modal::width(&text);
            let text = if target.is_none() && text.starts_with('…') {
                "…".to_owned()
            } else {
                text
            };
            let hit_width = super::modal::width(&text);
            let control = PathControl {
                area: Rect::new(x, area.y, hit_width, 1),
                pane,
                source: app.panes[pane].path.clone(),
                target,
                text,
            };
            x += width;
            (hit_width > 0).then_some(control)
        })
        .collect()
}

fn pieces(path: &std::path::Path, width: u16) -> Vec<(String, Option<PathBuf>)> {
    let (shown, leading, ending) = super::path_title::shortened(path, usize::from(width));
    let full = super::clean(&path.to_string_lossy());
    let segments = path_segments(path);
    let mut result = slices(&segments, 0, leading);
    if shown != full {
        let suffix_len = full.len() - ending;
        let middle = &shown[leading..shown.len() - suffix_len];
        result.push((middle.to_owned(), None));
        result.extend(slices(&segments, ending, full.len()));
    }
    result
}

fn slices(
    segments: &[crate::navigation::PathSegment],
    start: usize,
    end: usize,
) -> Vec<(String, Option<PathBuf>)> {
    let mut offset = 0;
    let mut result = Vec::new();
    for segment in segments {
        let right = offset + segment.text.len();
        let left = start.max(offset);
        let until = end.min(right);
        if left < until {
            result.push((
                segment.text[left - offset..until - offset].to_owned(),
                Some(segment.path.clone()),
            ));
        }
        offset = right;
    }
    result
}

pub(super) fn render(
    frame: &mut Frame,
    app: &App,
    theme: &Theme,
    controls: &[PathControl],
    pane: usize,
) {
    for control in controls.iter().filter(|c| c.pane == pane) {
        let hovered = app
            .path_pointer
            .is_some_and(|(x, y)| control.area.contains((x, y).into()));
        let style = if hovered {
            theme.style(Role::PathHover)
        } else if theme.level == colored_text::ColorLevel::NoColor {
            theme
                .style(Role::Normal)
                .remove_modifier(ratatui::style::Modifier::BOLD)
        } else {
            theme.style(Role::Normal)
        };
        frame.render_widget(
            Paragraph::new(control.text.clone()).style(style),
            control.area,
        );
    }
}
