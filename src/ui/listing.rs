use super::clean;
use crate::{
    app::{App, IgnoreMode, Pane},
    color::{Role, Theme},
    fs::{Entry, EntryKind},
};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

pub(super) fn render(frame: &mut Frame, app: &App, id: usize, area: Rect, theme: &Theme) {
    let pane = &app.panes[id];
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            "{} Pane {}: {}{}",
            if id == app.active { ">" } else { " " },
            id + 1,
            clean(&pane.path.to_string_lossy()),
            if pane.pending.is_some() {
                " [loading]"
            } else {
                ""
            }
        ))
        .border_style(theme.style(if id == app.active {
            Role::Header
        } else {
            Role::Normal
        }));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(2),
    );
    render_rows(frame, pane, rows_area, theme);
    render_summary(frame, pane, inner);
    render_status(frame, pane, inner, theme);
}

fn render_summary(frame: &mut Frame, pane: &Pane, inner: Rect) {
    let counts = pane.selection_counts();
    let summary = format!(
        "{} items | {} marked +{} excluded ({} hidden/{} ignored/{} filtered)",
        pane.visible.len(),
        counts.visible,
        counts.excluded(),
        counts.hidden,
        counts.ignored,
        counts.filtered
    );
    frame.render_widget(
        Paragraph::new(summary),
        Rect::new(
            inner.x,
            inner.bottom().saturating_sub(2),
            inner.width,
            inner.height.min(1),
        ),
    );
}

fn render_status(frame: &mut Frame, pane: &Pane, inner: Rect, theme: &Theme) {
    let status = pane
        .error
        .as_deref()
        .or_else(|| pane.warnings.first().map(String::as_str));
    let policy = format!(
        "/{}/ {:?}{} {:?} hidden:{} ignore:{:?}",
        clean(&pane.filter),
        pane.sort.sort,
        if pane.sort.reverse { " reverse" } else { "" },
        pane.sort.grouping,
        pane.show_hidden,
        pane.ignore
    );
    frame.render_widget(
        Paragraph::new(status.map_or(policy, clean)).style(theme.style(if status.is_some() {
            Role::Warning
        } else {
            Role::Normal
        })),
        Rect::new(
            inner.x,
            inner.bottom().saturating_sub(1),
            inner.width,
            inner.height.min(1),
        ),
    );
}

fn render_rows(frame: &mut Frame, pane: &Pane, area: Rect, theme: &Theme) {
    let height = usize::from(area.height.saturating_sub(1));
    let start = pane.viewport_start(height);
    let (widths, headers) = columns(area.width);
    let rows = pane.visible.iter().skip(start).take(height).map(|i| {
        let entry = &pane.entries[*i];
        let cursor = pane.cursor.as_ref() == Some(&entry.path);
        let style = theme.style(if cursor {
            Role::Cursor
        } else {
            role(entry, pane.ignore)
        });
        Row::new(cells(entry, pane, area.width)).style(style)
    });
    frame.render_widget(
        Table::new(rows, widths)
            .header(Row::new(headers).style(theme.style(Role::Header)))
            .column_spacing(1),
        area,
    );
    if pane.visible.is_empty() {
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
}

fn columns(width: u16) -> (Vec<Constraint>, Vec<&'static str>) {
    if width < 8 {
        return (vec![Constraint::Fill(1)], vec!["Name"]);
    }
    let mut widths = vec![
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Fill(1),
    ];
    let mut names = vec!["Mark", "T", "Name"];
    for (minimum, length, name) in [
        (45, 9, "Size"),
        (70, 16, "Modified UTC"),
        (90, 10, "Mode/attrs"),
        (125, 18, "Owner"),
    ] {
        if width >= minimum {
            widths.push(Constraint::Length(length));
            names.push(name);
        }
    }
    (widths, names)
}

fn cells(entry: &Entry, pane: &Pane, width: u16) -> Vec<Cell<'static>> {
    let name = format!(
        "{}{}",
        if entry.hidden { "[h] " } else { "" },
        clean(&entry.name.to_string_lossy())
    );
    if width < 8 {
        return vec![Cell::from(name)];
    }
    let mut cells = vec![
        Cell::from(if pane.marks.contains_key(&entry.path) {
            "[*]"
        } else {
            "   "
        }),
        Cell::from(indicator(entry)),
        Cell::from(name),
    ];
    if width >= 45 {
        cells.push(Cell::from(if entry.enterable {
            "—".into()
        } else {
            size(entry.size)
        }));
    }
    if width >= 70 {
        cells.push(Cell::from(timestamp(entry.modified)));
    }
    if width >= 90 {
        cells.push(Cell::from(entry.attributes.clone()));
    }
    if width >= 125 {
        cells.push(Cell::from(
            entry.owner.clone().unwrap_or_else(|| "unavailable".into()),
        ));
    }
    cells
}

fn indicator(entry: &Entry) -> &'static str {
    match entry.kind {
        EntryKind::Directory => "/",
        EntryKind::File if entry.executable => "*",
        EntryKind::File => "-",
        EntryKind::Link if entry.error.is_some() => "!",
        EntryKind::Link => "@",
        EntryKind::Junction => "j",
        EntryKind::ReparsePoint => "r",
        EntryKind::Socket => "s",
        EntryKind::Fifo => "p",
        EntryKind::Device => "d",
        EntryKind::Unknown => "?",
    }
}

fn role(entry: &Entry, ignore: IgnoreMode) -> Role {
    if entry.error.is_some() {
        Role::Warning
    } else if ignore == IgnoreMode::Dim && entry.ignored == Some(true) {
        Role::Ignored
    } else if matches!(
        entry.kind,
        EntryKind::Link | EntryKind::Junction | EntryKind::ReparsePoint
    ) {
        Role::Link
    } else if entry.enterable {
        Role::Directory
    } else if entry.executable {
        Role::Executable
    } else {
        Role::Normal
    }
}

pub(super) fn size(bytes: Option<u64>) -> String {
    let Some(bytes) = bytes else {
        return "unavailable".into();
    };
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", units[unit])
    }
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
        assert_eq!(
            timestamp(Some(UNIX_EPOCH + Duration::new(1, 123))),
            "1970-01-01 00:00:01.000000123"
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
