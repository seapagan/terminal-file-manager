use ratatui::{style::Style, text::Line};
use std::path::{Component, Path, PathBuf};

/// Truncate at grapheme boundaries, reserving one display cell for an ellipsis.
pub fn truncate(text: &str, cells: u16) -> String {
    if Line::raw(text).width() <= usize::from(cells) {
        return text.into();
    }
    let mut result = String::new();
    let mut used = 0;
    for grapheme in Line::raw(text).styled_graphemes(Style::default()) {
        let width = super::width(grapheme.symbol);
        if used + width > cells.saturating_sub(1) {
            break;
        }
        result.push_str(grapheme.symbol);
        used += width;
    }
    if cells > 0 {
        result.push('…');
    }
    result
}

/// Structured filename and indivisible size, right aligned in the available cells.
pub fn file_row(name: &str, size: &str, cells: u16) -> String {
    let size_width = super::width(size);
    if size_width > cells {
        return truncate(name, cells);
    }
    let name_width = cells.saturating_sub(size_width.saturating_add(1));
    let name = if name.ends_with('/') && name_width >= 5 && super::width(name) > name_width {
        format!("{}/", truncate(name.trim_end_matches('/'), name_width - 1))
    } else {
        truncate(name, name_width)
    };
    format!(
        "{name}{}{size}",
        " ".repeat(usize::from(
            cells
                .saturating_sub(super::width(&name))
                .saturating_sub(size_width)
        ))
    )
}

/// Lexical native common parent. Trivial volume roots are deliberately not compressed.
pub fn common_root<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Option<PathBuf> {
    let mut paths = paths.into_iter();
    let mut root = paths.next()?.parent()?.to_owned();
    for path in paths {
        while !path.starts_with(&root) {
            if !root.pop() {
                return None;
            }
        }
    }
    root.components()
        .any(|component| matches!(component, Component::Normal(_)))
        .then_some(root)
}

/// Explicit binary units for modal file rows; the docked metadata format stays unchanged.
pub fn file_size(bytes: Option<u64>) -> String {
    bytes.map_or_else(
        || "unavailable".into(),
        |bytes| {
            let (value, unit) = crate::size::show_size(bytes, Some(crate::size::SizeScale::Binary));
            let unit = match unit {
                "K" => "KiB",
                "M" => "MiB",
                "G" => "GiB",
                "T" => "TiB",
                "P" => "PiB",
                _ => unit,
            };
            format!("{value} {unit}")
        },
    )
}

/// Cached no-follow metadata only; formatting never inspects or resolves a path.
pub fn entry_row<T: Clone>(entry: &crate::fs::Entry, path: &Path) -> super::Row<T> {
    let suffix = if entry.kind == crate::fs::EntryKind::Directory
        || (entry.kind == crate::fs::EntryKind::Junction && entry.enterable)
    {
        "/"
    } else {
        ""
    };
    super::Row::file(
        format!(
            "{}  {}{suffix}",
            super::super::listing::indicator(entry),
            path.display()
        ),
        if entry.kind == crate::fs::EntryKind::File {
            crate::ui::modal::file_size(entry.size)
        } else {
            "—".into()
        },
    )
}
