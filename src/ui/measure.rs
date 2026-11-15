//! Cached listing measurements; recomputed with the visible working set.
use crate::{
    fs::{Entry, EntryKind},
    size::{SizeScale, show_size},
};
use ratatui::{style::Style, text::Line};

/// Natural content widths, bounded to the terminal's addressable columns.
#[derive(Clone, Debug)]
pub struct Measurements {
    pub name: u16,
    pub number: u16,
    pub owner: u16,
    pub attributes: u16,
    #[cfg(windows)]
    owner_widths: std::collections::BTreeMap<u16, usize>,
}

impl Default for Measurements {
    fn default() -> Self {
        Self {
            name: 0,
            number: 0,
            owner: 0,
            attributes: width(crate::permissions::PermissionDisplay::default().column_heading()),
            #[cfg(windows)]
            owner_widths: Default::default(),
        }
    }
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
            attributes: width(permissions.column_heading()),
            #[cfg(windows)]
            owner_widths: Default::default(),
        };
        for entry in entries {
            let name = name(entry, u16::MAX);
            measured.name = measured.name.max(width(&name));
            measured.attributes = measured
                .attributes
                .max(width(&crate::permissions::attributes(entry, permissions)));
            let (number, _) = parts(entry, human);
            measured.number = measured.number.max(width(&number));
            let owner = width(&super::clean(&entry.owner_display(owner_display)));
            measured.owner = measured.owner.max(owner);
            #[cfg(windows)]
            {
                *measured.owner_widths.entry(owner).or_default() += 1;
            }
        }
        measured
    }

    /// Adjust one resolved owner without rescanning the directory. Counts are
    /// bounded by u16 display widths and rebuilt by existing view transitions.
    #[cfg(windows)]
    pub(crate) fn update_owner(&mut self, before: &str, after: &str) {
        let before = width(&super::clean(before));
        let after = width(&super::clean(after));
        if before == after {
            return;
        }
        if let Some(count) = self.owner_widths.get_mut(&before) {
            *count -= 1;
            if *count == 0 {
                self.owner_widths.remove(&before);
            }
        }
        *self.owner_widths.entry(after).or_default() += 1;
        self.owner = self
            .owner_widths
            .last_key_value()
            .map_or(5, |(width, _)| (*width).max(5));
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
