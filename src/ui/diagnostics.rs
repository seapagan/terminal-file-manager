//! Observe the pane buffer before other overlays; never reconstruct effective styles.
use super::Geometry;
use crate::{
    app::App,
    color::{Role, Theme},
};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Clear, Paragraph},
};

mod presentation;
#[cfg(test)]
mod tests;

pub(super) fn area(bounds: Rect) -> Rect {
    let width = bounds.width.min(76);
    let height = bounds
        .height
        .saturating_sub(2)
        .clamp(1, 24)
        .min(bounds.height);
    Rect::new(
        bounds.right().saturating_sub(width),
        bounds.y,
        width,
        height,
    )
}

pub(super) fn report(
    buffer: &Buffer,
    app: &App,
    geometry: &Geometry,
    theme: &Theme,
) -> Vec<String> {
    let mut lines = vec![
        "CAPABILITY".into(),
        format!("Applied ColorLevel: {:?}", theme.level),
        "F12 hides; Alt+PageUp/Down or wheel scroll".into(),
        "Pane buffer BEFORE overlays; physical terminal RGB unmeasured.".into(),
        "default/Reset = terminal default / unknown; inherit = no override.".into(),
        "LAYOUT".into(),
        format!("Focus: pane {}", app.active + 1),
        format!(
            "Mode: dual={} · maximized={} · split={}%",
            app.dual, app.maximized, app.split_percent
        ),
        format!("Terminal: {} × {}", buffer.area.width, buffer.area.height),
    ];
    for id in 0..2 {
        pane(&mut lines, buffer, app, geometry, theme, id);
    }
    lines.push("STARTUP EVIDENCE".into());
    if theme.evidence.is_empty() {
        lines.push("Injected Theme level; startup detection evidence unavailable".into());
    } else {
        lines.extend(theme.evidence.iter().cloned());
    }
    lines
}

fn pane(
    lines: &mut Vec<String>,
    buffer: &Buffer,
    app: &App,
    geometry: &Geometry,
    theme: &Theme,
    id: usize,
) {
    let pane = &app.panes[id];
    intended(lines, app, theme, id);
    let Some(area) = geometry.panes[id] else {
        lines.push("Hidden pane: rendered cells unavailable".into());
        return;
    };
    lines.push("RENDERED CELLS".into());
    lines.push(format!(
        "Content: {} columns · {} rows",
        area.width.saturating_sub(2),
        geometry.listing_rows[id]
    ));
    lines.push(format!(
        "Bounds: x={} y={} width={} height={}",
        area.x, area.y, area.width, area.height
    ));
    sample(
        lines,
        buffer,
        area,
        "border rendered",
        area.x,
        area.y.saturating_add(1),
    );
    sample(
        lines,
        buffer,
        area,
        "title rendered",
        area.x.saturating_add(1),
        area.y,
    );
    body_sample(lines, buffer, pane, area, geometry.listing_rows[id]);
    selected_row(lines, buffer, pane, area, geometry.listing_rows[id]);
}

fn intended(lines: &mut Vec<String>, app: &App, theme: &Theme, id: usize) {
    let pane = &app.panes[id];
    let active = id == app.active;
    let body = theme.style(if active {
        Role::ActivePane
    } else {
        Role::Normal
    });
    let border = theme.style(if active { Role::Header } else { Role::Normal });
    let cursor = theme.style(if active {
        Role::Cursor
    } else {
        Role::InactiveCursor
    });
    lines.push(format!(
        "Pane {} · {}",
        id + 1,
        if active { "ACTIVE" } else { "INACTIVE" },
    ));
    lines.push(format!("Profile: {}", pane.profile_label()));
    lines.push(format!(
        "Listing: {} entries · {} visible · {} marked",
        pane.entries.len(),
        pane.visible.len(),
        pane.marks.len()
    ));
    lines.push(format!(
        "Cursor: {}",
        pane.cursor.as_ref().map_or_else(
            || "none".into(),
            |path| super::clean(&path.to_string_lossy())
        )
    ));
    lines.push("INTENDED STYLES".into());
    for (label, style) in [
        ("body intended", body),
        ("border/title intended", border),
        ("selected row intended", cursor),
    ] {
        lines.push(format!("{label}: {}", describe(style)));
    }
}

fn body_sample(
    lines: &mut Vec<String>,
    buffer: &Buffer,
    pane: &crate::app::Pane,
    area: Rect,
    rows: usize,
) {
    let start = pane.viewport_start(rows);
    let used = pane.visible.len().saturating_sub(start).min(rows);
    if area.width > 2 && used < rows {
        sample(
            lines,
            buffer,
            area,
            "unoccupied body rendered",
            area.x + 1,
            area.y + 2 + used as u16,
        );
    } else {
        lines.push("Unoccupied body sample unavailable: no blank listing row".into());
    }
}

fn selected_row(
    lines: &mut Vec<String>,
    buffer: &Buffer,
    pane: &crate::app::Pane,
    area: Rect,
    rows: usize,
) {
    let start = pane.viewport_start(rows);
    let position = pane
        .visible
        .iter()
        .position(|index| Some(&pane.entries[*index].path) == pane.cursor.as_ref());
    let Some(offset) = position
        .and_then(|position| position.checked_sub(start))
        .filter(|offset| *offset < rows)
    else {
        lines.push("Selected row rendered: unavailable (no visible cursor row)".into());
        return;
    };
    let y = area.y + 2 + offset as u16;
    let mut x = area.x.saturating_add(1);
    let right = area.right().saturating_sub(1);
    while x < right {
        let Some(cell) = buffer.cell((x, y)) else {
            break;
        };
        let style = cell.style();
        let first = x;
        x += 1;
        while x < right
            && buffer
                .cell((x, y))
                .is_some_and(|cell| cell.style() == style)
        {
            x += 1;
        }
        lines.push(format!(
            "selected rendered x={first}..{x} y={y} ({} cells): {}",
            x - first,
            describe(style)
        ));
    }
}

fn sample(lines: &mut Vec<String>, buffer: &Buffer, area: Rect, label: &str, x: u16, y: u16) {
    if area.contains((x, y).into())
        && let Some(cell) = buffer.cell((x, y))
    {
        lines.push(format!("{label} ({x},{y}): {}", describe(cell.style())));
    } else {
        lines.push(format!("{label}: unavailable (outside pane/frame)"));
    }
}

fn colour(colour: Option<Color>) -> String {
    match colour {
        None => "inherit".into(),
        Some(Color::Reset) => "terminal default / unknown (Reset)".into(),
        Some(Color::Rgb(r, g, b)) => format!("RGB({r},{g},{b})"),
        Some(Color::Indexed(index)) => format!("indexed({index})"),
        Some(named) => format!("ANSI({named:?})"),
    }
}

fn describe(style: Style) -> String {
    format!(
        "fg={} bg={} underline={} modifiers={:?} remove={:?}",
        colour(style.fg),
        colour(style.bg),
        colour(style.underline_color),
        style.add_modifier,
        style.sub_modifier
    )
}

pub(super) fn render(
    frame: &mut Frame,
    app: &App,
    theme: &Theme,
    geometry: &mut Geometry,
    lines: Vec<String>,
) {
    let area = geometry.diagnostics.expect("visible diagnostics");
    if area.is_empty() {
        return;
    }
    // Borderless at tiny sizes so even one display cell remains scrollable.
    let block = if area.width >= 3 && area.height >= 3 {
        Block::bordered()
            .title("Diagnostics — F12")
            .border_style(theme.style(Role::Header))
    } else {
        Block::default()
    };
    let inner = block.inner(area);
    let wrapped: Vec<_> = presentation::lines(lines, theme)
        .into_iter()
        .flat_map(|line| super::text::wrap_line(line, usize::from(inner.width)))
        .collect();
    geometry.diagnostics_scroll_limit =
        u16::try_from(wrapped.len().saturating_sub(usize::from(inner.height))).unwrap_or(u16::MAX);
    let scroll = app
        .diagnostics_scroll
        .min(geometry.diagnostics_scroll_limit);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(wrapped).scroll((scroll, 0)).block(block),
        area,
    );
}
