//! Cached listing measurements; recomputed with the visible working set.
use crate::{
    fs::Entry,
    size::{SizeScale, show_size},
};
use ratatui::text::Line;

/// Natural content widths, bounded to the terminal's addressable columns.
#[derive(Clone, Debug, Default)]
pub struct Measurements {
    pub name: u16,
    pub number: u16,
    pub owner: u16,
}

impl Measurements {
    /// Measure display cells after control escaping; native identities remain untouched.
    pub fn collect<'a>(entries: impl Iterator<Item = &'a Entry>, human: bool) -> Self {
        let mut measured = Self {
            name: 4,
            number: 4,
            owner: 5,
        };
        for entry in entries {
            let name = format!(
                "{}{}",
                if entry.hidden { "[h] " } else { "" },
                super::clean(&entry.name.to_string_lossy())
            );
            measured.name = measured.name.max(width(&name));
            let (number, _) = parts(entry, human);
            measured.number = measured.number.max(width(&number));
            measured.owner = measured
                .owner
                .max(width(entry.owner.as_deref().unwrap_or("unavailable")));
        }
        measured
    }
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
