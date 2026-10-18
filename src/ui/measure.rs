//! Cached listing measurements; recomputed with the visible working set.
use crate::{
    fs::{Entry, EntryKind},
    size::{SizeScale, show_size},
};
use ratatui::{style::Style, text::Line};

/// Natural content widths, bounded to the terminal's addressable columns.
#[derive(Clone, Debug, Default)]
pub struct Measurements {
    pub name: u16,
    pub number: u16,
    pub owner: u16,
    pub attributes: u16,
}

impl Measurements {
    /// Measure display cells after control escaping; native identities remain untouched.
    pub fn collect<'a>(
        entries: impl Iterator<Item = &'a Entry>,
        human: bool,
        permissions: crate::permissions::PermissionDisplay,
        owner_display: crate::owner::OwnerDisplay,
    ) -> Self {
        let mut measured = Self {
            name: 4,
            number: 4,
            owner: 5,
            attributes: 10,
        };
        for entry in entries {
            let name = name(entry, u16::MAX);
            measured.name = measured.name.max(width(&name));
            measured.attributes = measured
                .attributes
                .max(width(&crate::permissions::attributes(entry, permissions)));
            let (number, _) = parts(entry, human);
            measured.number = measured.number.max(width(&number));
            measured.owner = measured
                .owner
                .max(width(&super::clean(&entry.owner_display(owner_display))));
        }
        measured
    }
}

pub(super) fn name(entry: &Entry, width: u16) -> String {
    let name = super::clean(&entry.name.to_string_lossy());
    if entry.kind != EntryKind::Directory {
        return name;
    }
    let mut visible = String::new();
    let mut used = 0;
    let line = Line::raw(name);
    for grapheme in line.styled_graphemes(Style::default()) {
        let cells = Line::raw(grapheme.symbol).width();
        if used + cells > usize::from(width.saturating_sub(1)) {
            break;
        }
        visible.push_str(grapheme.symbol);
        used += cells;
    }
    if width > 0 {
        visible.push('/');
    }
    visible
}

fn width(text: &str) -> u16 {
    u16::try_from(Line::raw(text).width()).unwrap_or(u16::MAX)
}

pub(super) fn parts(entry: &Entry, human: bool) -> (String, &'static str) {
    if entry.enterable {
        ("—".into(), "")
    } else {
        entry.size.map_or_else(
            || ("unavailable".into(), ""),
            |bytes| show_size(bytes, human.then_some(SizeScale::Binary)),
        )
    }
}
