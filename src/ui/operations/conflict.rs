use super::{
    content::{self, Row},
    layout::Layout,
};
use crate::{
    app::App,
    color::Role,
    fs::{Entry, EntryKind},
    operations::Event,
    ui::modal,
};
use ratatui::layout::Rect;
use std::{path::Path, time::SystemTime};

pub(super) fn title(app: &App) -> &'static str {
    if let Some(conflict) = app.operations.conflicts.front()
        && let Event::Conflict {
            source,
            destination,
            ..
        } = &conflict.event
        && source.kind == EntryKind::File
        && destination.kind == EntryKind::File
    {
        "File already exists"
    } else {
        "Destination already exists"
    }
}

pub(super) fn layout(app: &App, terminal: Rect) -> Option<Layout> {
    let conflict = app.operations.conflicts.front()?;
    let Event::Conflict {
        source,
        destination,
        newer_available,
    } = &conflict.event
    else {
        return None;
    };
    let desired = preferred_width(source, destination, *newer_available)
        .saturating_add(6)
        .min(terminal.width);
    let available = Rect {
        x: terminal.x + (terminal.width - desired) / 2,
        width: desired,
        ..terminal
    };
    let actions = super::layout::buttons(app, terminal.width);
    let mut spec = modal::Spec {
        title: content::title(app),
        content_width: usize::from(desired),
        pinned_rows: 0,
        footer_rows: 0,
        actions: &actions,
    };
    let width = spec.measure(available, |_| 0).body.width;
    let header = content::wrapped(header(source, destination, width), width);
    let rows = content::wrapped(
        comparison(source, destination, *newer_available, width),
        width,
    );
    let footer = content::wrapped(footer(source, destination, *newer_available), width);
    spec.pinned_rows = header.len() as u16;
    spec.footer_rows = footer.len() as u16;
    let shell = spec.measure(available, |_| rows.len());
    Some(Layout::measured(app, shell, header, footer, rows))
}

fn preferred_width(source: &Entry, destination: &Entry, trusted: bool) -> u16 {
    let root = modal::common_root([source.path.as_path(), destination.path.as_path()]);
    let column = [source, destination]
        .into_iter()
        .flat_map(|entry| facts(entry, root.as_deref(), u16::MAX))
        .map(|(_, value, _)| modal::width(&value))
        .max()
        .unwrap_or(0)
        .max(modal::width("Existing (destination)"));
    header(source, destination, u16::MAX)
        .into_iter()
        .chain(footer(source, destination, trusted))
        .map(|row| row.width().min(usize::from(u16::MAX)) as u16)
        .chain([13u16.saturating_add(column.saturating_mul(2))])
        .chain([
            modal::width(&direction(source.modified, destination.modified, trusted))
                .saturating_add(7),
        ])
        .max()
        .unwrap_or(0)
}

fn header(source: &Entry, destination: &Entry, width: u16) -> Vec<Row> {
    let mut rows = if source.name == destination.name {
        vec![Row::styled(name(source), Role::Header).single_line()]
    } else {
        vec![
            Row::key_value("Incoming", name(source)).single_line(),
            Row::key_value("Existing", name(destination)).single_line(),
        ]
    };
    if let Some(root) = modal::common_root([source.path.as_path(), destination.path.as_path()]) {
        rows.push(
            Row::key_value(
                "Root",
                crate::ui::path_title::comparison(&root, width.saturating_sub(6)),
            )
            .single_line(),
        );
    }
    rows
}

fn comparison(source: &Entry, destination: &Entry, trusted: bool, width: u16) -> Vec<Row> {
    let root = modal::common_root([source.path.as_path(), destination.path.as_path()]);
    let mut rows = if width >= 59 {
        columns(source, destination, root.as_deref(), width)
    } else {
        let mut rows = section("Incoming (source)", source, root.as_deref(), width);
        rows.extend(section(
            "Existing (destination)",
            destination,
            root.as_deref(),
            width,
        ));
        rows
    };
    rows.push(Row::key_value(
        "Newer",
        direction(source.modified, destination.modified, trusted),
    ));
    rows
}

fn columns(source: &Entry, destination: &Entry, root: Option<&Path>, width: u16) -> Vec<Row> {
    let column = (width - 13) / 2;
    let mut rows = vec![pair(
        "",
        "Incoming (source)",
        "Existing (destination)",
        column,
        Role::Label,
    )];
    for ((key, incoming, role), (_, existing, _)) in facts(source, root, column)
        .into_iter()
        .zip(facts(destination, root, column))
    {
        rows.push(pair(key, &incoming, &existing, column, role));
    }
    rows
}

fn pair(key: &str, incoming: &str, existing: &str, column: u16, role: Role) -> Row {
    let gap = column
        .saturating_sub(modal::width(incoming))
        .saturating_add(3);
    let mut row = Row::key_value(
        key,
        format!("{incoming}{}{existing}", " ".repeat(usize::from(gap))),
    )
    .label_width(10);
    row.role = role;
    row
}

fn section(title: &str, entry: &Entry, root: Option<&Path>, width: u16) -> Vec<Row> {
    let mut rows = vec![Row::styled(title, Role::Label)];
    for (key, value, role) in facts(entry, root, width.saturating_sub(10)) {
        let mut row = Row::key_value(key, value).label_width(10);
        row.role = role;
        rows.push(row);
    }
    rows
}

fn facts(entry: &Entry, root: Option<&Path>, width: u16) -> [(&'static str, String, Role); 4] {
    let path = relative(&entry.path, root);
    let directory = entry.kind == EntryKind::Directory
        || (entry.kind == EntryKind::Junction && entry.enterable);
    let mut shown =
        crate::ui::path_title::comparison(path, width.saturating_sub(u16::from(directory)));
    if directory {
        shown.push('/');
    }
    [
        ("Path", shown, Role::Value),
        ("Type", kind(entry), Role::Value),
        (
            "Size",
            if entry.kind == EntryKind::File {
                modal::file_size(entry.size)
            } else {
                "—".into()
            },
            Role::Status,
        ),
        ("Modified", modified(entry.modified), Role::Value),
    ]
}

fn relative<'a>(path: &'a Path, root: Option<&Path>) -> &'a Path {
    root.and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path)
}

fn name(entry: &Entry) -> String {
    entry
        .name
        .to_str()
        .map_or_else(|| format!("{:?}", entry.name), crate::ui::clean)
}

fn kind(entry: &Entry) -> String {
    let name = match entry.kind {
        EntryKind::Directory => "Directory",
        EntryKind::File => "File",
        EntryKind::Link => "Link",
        EntryKind::Junction => "Junction",
        EntryKind::ReparsePoint => "Reparse point",
        EntryKind::Socket => "Socket",
        EntryKind::Fifo => "FIFO",
        EntryKind::Device => "Device",
        EntryKind::Unknown => "Unknown",
    };
    format!("{} {name}", crate::ui::listing::indicator(entry))
}

fn modified(time: Option<SystemTime>) -> String {
    let full = crate::ui::listing::timestamp(time);
    full.split_once('.')
        .map_or_else(|| full.clone(), |(seconds, _)| format!("{seconds} UTC"))
}

fn direction(incoming: Option<SystemTime>, existing: Option<SystemTime>, trusted: bool) -> String {
    let (Some(incoming), Some(existing)) = (incoming, existing) else {
        return "Cannot compare reliably".into();
    };
    if !trusted {
        return "Cannot compare reliably".into();
    }
    let (label, distance) = match incoming.cmp(&existing) {
        std::cmp::Ordering::Greater => {
            ("Incoming newer", incoming.duration_since(existing).unwrap())
        }
        std::cmp::Ordering::Less => ("Existing newer", existing.duration_since(incoming).unwrap()),
        std::cmp::Ordering::Equal => return "Same modified time".into(),
    };
    format!(
        "{label}{}",
        if distance.as_secs() == 0 {
            " (<1 s)"
        } else {
            ""
        }
    )
}

fn footer(source: &Entry, destination: &Entry, newer: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    if source.may_recurse() != destination.may_recurse() {
        rows.push(Row::styled(
            "Replace disabled: types differ.",
            Role::Warning,
        ));
    }
    if !newer {
        rows.push(Row::styled(
            if source.kind != destination.kind {
                "Newer disabled: types differ."
            } else if source.modified.is_some() && destination.modified.is_some() {
                "Newer disabled: unverified times."
            } else {
                "Newer disabled: time unavailable."
            },
            Role::Disabled,
        ));
    }
    rows
}
