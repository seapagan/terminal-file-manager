//! Width-aware presentation only; native paths never leave the pane model.
use std::path::{Component, Path};

use ratatui::{style::Style, text::Line};

pub(super) fn title(path: &Path, id: usize, active: bool, loading: bool, width: u16) -> String {
    let width = usize::from(width);
    let mut label = format!("{} Pane {}: ", if active { ">" } else { " " }, id + 1);
    let suffix = if loading { " [loading]" } else { "" };
    if label.len() + suffix.len() > width {
        label.truncate(width.min(label.len()));
        return label;
    }
    let path = shorten(path, width - label.len() - suffix.len());
    format!("{label}{path}{suffix}")
}

fn shorten(path: &Path, width: usize) -> String {
    let raw = path.to_string_lossy();
    let full = super::clean(&raw);
    if cells(&full) <= width {
        return full;
    }
    let (anchor, parts, separator) = components(path, &raw);
    let Some(last) = parts.last() else {
        return tail(&full, width);
    };
    if parts.len() < 2 || cells(last) + 2 > width {
        return tail(last, width);
    }
    let anchor = if cells(&anchor) + cells(last) + 2 <= width {
        anchor
    } else {
        String::new()
    };
    let mut used = cells(&anchor) + cells(last) + 2;
    let mut end = parts.len() - 1;
    while end > 1 && used + cells(&parts[end - 1]) <= width {
        end -= 1;
        used += cells(&parts[end]);
    }
    let mut start = 0;
    while start + 1 < end && used + cells(&parts[start]) <= width {
        used += cells(&parts[start]);
        start += 1;
    }
    format!(
        "{anchor}{}…{separator}{}",
        parts[..start].concat(),
        parts[end..].concat()
    )
}

fn components(path: &Path, raw: &str) -> (String, Vec<String>, char) {
    // Use native prefix semantics, then retain the original separators and trailing spelling.
    let prefix = match path.components().next() {
        Some(Component::Prefix(prefix)) => prefix.as_os_str().to_string_lossy().len(),
        _ => 0,
    };
    let verbatim =
        matches!(path.components().next(), Some(Component::Prefix(p)) if p.kind().is_verbatim());
    let is_separator = |c| {
        if verbatim {
            c == '\\'
        } else {
            std::path::is_separator(c)
        }
    };
    let root = prefix
        + raw[prefix..]
            .chars()
            .take_while(|c| is_separator(*c))
            .map(char::len_utf8)
            .sum::<usize>();
    let mut starts = Vec::new();
    let mut boundary = true;
    let mut separator = std::path::MAIN_SEPARATOR;
    for (offset, c) in raw[root..].char_indices() {
        if is_separator(c) {
            separator = c;
            boundary = true;
        } else {
            if boundary {
                starts.push(root + offset);
            }
            boundary = false;
        }
    }
    starts.push(raw.len());
    let parts = starts
        .windows(2)
        .map(|pair| super::clean(&raw[pair[0]..pair[1]]))
        .collect();
    (super::clean(&raw[..root]), parts, separator)
}

fn cells(text: &str) -> usize {
    Line::raw(text).width()
}

fn tail(text: &str, width: usize) -> String {
    if cells(text) == width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let line = Line::raw(text);
    let graphemes = line.styled_graphemes(Style::default()).collect::<Vec<_>>();
    let mut used = 1;
    let mut start = graphemes.len();
    for grapheme in graphemes.iter().rev() {
        let length = cells(grapheme.symbol);
        if used + length > width {
            break;
        }
        used += length;
        start -= 1;
    }
    let mut result = String::from("…");
    for grapheme in &graphemes[start..] {
        result.push_str(grapheme.symbol);
    }
    result
}
