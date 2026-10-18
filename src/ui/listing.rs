use super::clean;
use crate::{
    app::{App, IgnoreMode, Pane},
    color::{Role, Theme},
    fs::{Entry, EntryKind},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

pub(super) fn render(
    frame: &mut Frame,
    app: &App,
    id: usize,
    geometry: &super::Geometry,
    theme: &Theme,
) {
    let pane = &app.panes[id];
    let area = geometry.panes[id].expect("visible pane");
    let block = Block::default()
        .style(theme.style(if id == app.active {
            Role::ActivePane
        } else {
            Role::Normal
        }))
        .borders(Borders::ALL)
        .border_style(theme.style(if id == app.active {
            Role::Header
        } else {
            Role::Normal
        }));
    let inner = block.inner(area);
    let title = super::path_title::title(
        &pane.path,
        id,
        id == app.active,
        pane.pending.is_some(),
        inner.width,
    );
    frame.render_widget(block.title(title), area);
    let summary = super::summary::lines(
        pane.visible.len(),
        geometry.selection_counts[id],
        inner.width,
        theme,
    );
    let rows_area = Rect {
        height: inner.height.saturating_sub(summary.len() as u16 + 1),
        ..inner
    };
    render_rows(frame, pane, rows_area, theme, id == app.active, app.now);
    render_summary(frame, summary, inner);
    render_status(frame, pane, inner, theme);
}

fn render_summary(frame: &mut Frame, lines: Vec<Line<'static>>, inner: Rect) {
    let height = (lines.len() as u16).min(inner.height.saturating_sub(1));
    frame.render_widget(
        Paragraph::new(lines),
        Rect::new(
            inner.x,
            inner.bottom().saturating_sub(height + 1),
            inner.width,
            height,
        ),
    );
}

fn render_status(frame: &mut Frame, pane: &Pane, inner: Rect, theme: &Theme) {
    let status = pane
        .error
        .as_deref()
        .or_else(|| pane.warnings.first().map(String::as_str));
    let line = status.map_or_else(
        || super::policy::line(pane, inner.width, theme),
        |message| Line::styled(clean(message), theme.style(Role::Warning)),
    );
    frame.render_widget(
        Paragraph::new(line),
        Rect::new(
            inner.x,
            inner.bottom().saturating_sub(1),
            inner.width,
            inner.height.min(1),
        ),
    );
}

fn render_rows(
    frame: &mut Frame,
    pane: &Pane,
    area: Rect,
    theme: &Theme,
    active: bool,
    now: std::time::SystemTime,
) {
    let height = usize::from(area.height.saturating_sub(1));
    let start = pane.viewport_start(height);
    let (widths, headers) = columns(pane, area.width);
    let name_width = match widths.get(if area.width < 8 { 0 } else { 2 }) {
        Some(Constraint::Length(width)) => *width,
        _ => 0,
    };
    let rows = pane.visible.iter().skip(start).take(height).map(|i| {
        let entry = &pane.entries[*i];
        let cursor = pane.cursor.as_ref() == Some(&entry.path);
        let focus = if cursor && active {
            Some(Role::Cursor)
        } else if cursor {
            Some(Role::InactiveCursor)
        } else {
            None
        };
        let style = focus.map_or_else(Style::default, |role| theme.style(role));
        Row::new(cells(
            entry, pane, area.width, name_width, now, theme, focus, active,
        ))
        .style(style)
    });
    frame.render_widget(
        Table::new(rows, widths)
            .header(Row::new(headers).style(theme.style(Role::Header)))
            .column_spacing(1),
        area,
    );
    if pane.visible.is_empty() {
        render_empty(frame, pane, area);
    }
}

fn render_empty(frame: &mut Frame, pane: &Pane, area: Rect) {
    frame.render_widget(
        Paragraph::new(if pane.pending.is_some() {
            "Loading directory…"
        } else {
            "No visible entries"
        }),
        Rect::new(
            area.x,
            area.y.saturating_add(1),
            area.width,
            area.height.saturating_sub(1),
        ),
    );
}

fn columns(pane: &Pane, width: u16) -> (Vec<Constraint>, Vec<Cell<'static>>) {
    let columns = pane.columns.visible(width, &pane.measurements);
    if width < 8 {
        return (vec![Constraint::Length(width)], vec![Cell::from("Name")]);
    }
    let mut widths = vec![Constraint::Length(3), Constraint::Length(2)];
    let mut headers = vec![Cell::from(""), Cell::from("T"), Cell::from("Name")];
    let mut details = Vec::new();
    if columns.size {
        details.extend([
            Constraint::Length(pane.measurements.number),
            Constraint::Length(1),
        ]);
        headers.push(Cell::from(
            ratatui::text::Line::from("Size").alignment(Alignment::Right),
        ));
        headers.push(Cell::from(""));
    }
    for (length, name, enabled) in [
        (16, modified_header(pane.fuzzy_timestamps), columns.modified),
        (
            pane.measurements.attributes,
            "Mode/attrs",
            columns.attributes,
        ),
        (pane.measurements.owner.min(24), "Owner", columns.owner),
    ] {
        if enabled {
            details.push(Constraint::Length(length));
            headers.push(Cell::from(name));
        }
    }
    let reserved: u16 = details
        .iter()
        .map(|c| match c {
            Constraint::Length(n) => *n,
            _ => 0,
        })
        .sum();
    let spacing = (headers.len() - 1) as u16;
    let available = width.saturating_sub(5 + reserved + spacing);
    widths.push(Constraint::Length(
        pane.measurements.name.clamp(16, 64).min(available),
    ));
    widths.extend(details);
    (widths, headers)
}

fn modified_header(fuzzy: bool) -> &'static str {
    if fuzzy { "Modified" } else { "Modified UTC" }
}

fn cells(
    entry: &Entry,
    pane: &Pane,
    width: u16,
    name_width: u16,
    now: std::time::SystemTime,
    theme: &Theme,
    focus: Option<Role>,
    active: bool,
) -> Vec<Cell<'static>> {
    let columns = pane.columns.visible(width, &pane.measurements);
    let decorate = |base: Style| focus.map_or(base, |role| base.patch(theme.style(role)));
    if width < 8 {
        let mut cells = identity_cells(entry, pane, theme, focus, name_width, decorate, active);
        return vec![cells.pop().expect("name cell")];
    }
    let mut cells = identity_cells(entry, pane, theme, focus, name_width, decorate, active);
    if columns.size {
        let (number, unit) = super::measure::parts(entry, pane.human_sizes);
        cells.push(
            Cell::from(ratatui::text::Line::from(number).alignment(Alignment::Right))
                .style(decorate(theme.size_style(entry.size))),
        );
        cells.push(Cell::from(unit).style(decorate(theme.size_style(entry.size))));
    }
    if columns.modified {
        cells.push(
            Cell::from(listing_timestamp(
                entry.modified,
                pane.fuzzy_timestamps,
                now,
            ))
            .style(decorate(theme.time_style(entry.modified, now))),
        );
    }
    if columns.attributes {
        cells.push(permission_cell(entry, pane.permissions, theme, decorate));
    }
    if columns.owner {
        cells.push(
            Cell::from(clean(&entry.owner_display(pane.owner_display)))
                .style(decorate(theme.style(Role::Label))),
        );
    }
    cells
}

fn identity_cells(
    entry: &Entry,
    pane: &Pane,
    theme: &Theme,
    focus: Option<Role>,
    name_width: u16,
    decorate: impl Fn(Style) -> Style,
    active: bool,
) -> Vec<Cell<'static>> {
    let type_style = decorate(theme.entry_style(entry));
    let name_style = decorate(
        if pane.ignore == IgnoreMode::Dim && entry.ignored == Some(true) {
            theme.ignored_name_style(entry, active)
        } else {
            theme.entry_style(entry)
        },
    );
    let marked = pane.marks.contains_key(&entry.path);
    let marker = match (focus, marked) {
        (Some(Role::Cursor | Role::InactiveCursor), true) => " * ",
        (_, true) => "[*]",
        _ => "   ",
    };
    let name = super::measure::name(entry, name_width);
    vec![
        Cell::from(marker).style(decorate(theme.style(if marked {
            Role::Marked
        } else {
            Role::Normal
        }))),
        Cell::from(Line::from(vec![
            Span::styled(indicator(entry), type_style),
            Span::styled(
                if entry.hidden { "h" } else { " " },
                decorate(theme.style(Role::Hidden)),
            ),
        ])),
        Cell::from(Line::from(Span::styled(name, name_style))),
    ]
}

fn permission_cell(
    entry: &Entry,
    permissions: crate::permissions::PermissionDisplay,
    theme: &Theme,
    decorate: impl Fn(Style) -> Style,
) -> Cell<'static> {
    let spans = permission_spans(entry, permissions, theme)
        .into_iter()
        .map(|mut span| {
            span.style = decorate(span.style);
            span
        })
        .collect::<Vec<_>>();
    Cell::from(Line::from(spans))
}

pub(super) fn permission_spans(
    entry: &Entry,
    permissions: crate::permissions::PermissionDisplay,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let text = crate::permissions::attributes(entry, permissions);
    text.chars()
        .enumerate()
        .map(|(index, value)| {
            let style = if index == 0 {
                theme.entry_style(entry)
            } else if entry.mode.is_some()
                && permissions == crate::permissions::PermissionDisplay::Symbolic
            {
                theme.permission_style(value)
            } else {
                theme.style(Role::Warning)
            };
            Span::styled(value.to_string(), style)
        })
        .collect()
}

pub(super) fn listing_timestamp(
    time: Option<std::time::SystemTime>,
    fuzzy: bool,
    now: std::time::SystemTime,
) -> String {
    if fuzzy {
        time.map_or_else(
            || "unavailable".into(),
            |time| crate::fuzzy_time::fuzzy_time(time, now),
        )
    } else {
        timestamp(time)
    }
}

fn indicator(entry: &Entry) -> &'static str {
    match entry.kind {
        EntryKind::Directory => "d",
        EntryKind::File if entry.executable => "*",
        EntryKind::File => "-",
        EntryKind::Link if entry.error.is_some() => "!",
        EntryKind::Link => "@",
        EntryKind::Junction => "j",
        EntryKind::ReparsePoint => "r",
        EntryKind::Socket => "s",
        EntryKind::Fifo => "p",
        EntryKind::Device => "D",
        EntryKind::Unknown => "?",
    }
}

pub(super) fn size(bytes: Option<u64>) -> String {
    bytes.map_or_else(
        || "unavailable".into(),
        |bytes| {
            let (value, unit) = crate::size::show_size(bytes, Some(crate::size::SizeScale::Binary));
            format!("{value} {unit}")
        },
    )
}

pub(super) fn timestamp(time: Option<std::time::SystemTime>) -> String {
    let Some(time) = time else {
        return "unavailable".into();
    };
    let nanos = match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() as i128,
        Err(error) => -(error.duration().as_nanos() as i128),
    };
    let Ok(date) = time::OffsetDateTime::from_unix_timestamp_nanos(nanos) else {
        return "outside supported date range".into();
    };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:09}",
        date.year(),
        date.month() as u8,
        date.day(),
        date.hour(),
        date.minute(),
        date.second(),
        date.nanosecond()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_retain_precision_and_out_of_range_metadata_cannot_panic() {
        use std::time::{Duration, UNIX_EPOCH};
        // Windows SystemTime stores FILETIME ticks of 100 ns, before formatting.
        let fractional = if cfg!(windows) { "100" } else { "123" };
        assert_eq!(
            timestamp(Some(UNIX_EPOCH + Duration::new(1, 123))),
            format!("1970-01-01 00:00:01.000000{fractional}")
        );
        assert_eq!(
            timestamp(Some(UNIX_EPOCH + Duration::new(1, 123_456_700))),
            "1970-01-01 00:00:01.123456700"
        );
        assert_eq!(
            timestamp(Some(UNIX_EPOCH - Duration::from_secs(1))),
            "1969-12-31 23:59:59.000000000"
        );
        let extreme = UNIX_EPOCH
            .checked_add(Duration::from_secs(253_402_300_800))
            .unwrap();
        assert_eq!(timestamp(Some(extreme)), "outside supported date range");
    }
}
