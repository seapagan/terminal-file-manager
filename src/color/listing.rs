//! Ratatui adaptation of lsplus file.rs/render.rs classification (MIT, Grant Ramsay).
use super::{Role, Theme, map_color};
use crate::{
    fs::{Entry, EntryKind},
    fuzzy_time::{DAY, MONTH, WEEK, YEAR},
};
use colored_text::{AnsiColor, ColorLevel, resolve_ansi256};
use ratatui::style::{Modifier, Style};
use std::time::SystemTime;

impl Theme {
    /// Keep faint effective on VTE, which ignores it on direct RGB foregrounds.
    /// Cursor styling is applied afterwards and retains precedence.
    pub fn ignored_name_style(&self, entry: &Entry, active: bool) -> Style {
        let mut style = self.entry_style(entry).patch(self.style(Role::Ignored));
        if self.vte && self.level == ColorLevel::TrueColor {
            let inherited = active
                .then(|| self.style(Role::ActivePane))
                .and_then(|s| s.fg);
            if let Some(ratatui::style::Color::Rgb(r, g, b)) = style.fg.or(inherited) {
                style.fg = colored_text::resolve_rgb(r, g, b, ColorLevel::Ansi256).map(map_color);
            }
        }
        style
    }

    /// VTE needs indexed cyan for faint hidden markers; blank cells keep their style.
    pub(crate) fn hidden_indicator_style(&self, hidden: bool) -> Style {
        let mut style = self.style(Role::Hidden);
        if hidden
            && self.vte
            && self.level == ColorLevel::TrueColor
            && let Some(ratatui::style::Color::Rgb(r, g, b)) = style.fg
        {
            style.fg = colored_text::resolve_rgb(r, g, b, ColorLevel::Ansi256).map(map_color);
        }
        style
    }

    fn named(&self, color: AnsiColor) -> Style {
        self.accent(color)
            .map_or_else(Style::default, |color| Style::default().fg(color))
    }

    /// Preserve lsplus name classes; errors stay visible above type accents.
    pub fn entry_style(&self, entry: &Entry) -> Style {
        use EntryKind::*;
        if entry.error.is_some() {
            return self.style(Role::Warning);
        }
        match entry.kind {
            Link => self.named(AnsiColor::Cyan),
            Junction => self.named(AnsiColor::Magenta),
            Directory => self.named(AnsiColor::Blue),
            File if entry.executable => self.named(AnsiColor::Green).add_modifier(Modifier::BOLD),
            Socket => self.named(AnsiColor::Magenta).add_modifier(Modifier::BOLD),
            Fifo => self.named(AnsiColor::Yellow),
            Device => self.named(AnsiColor::Yellow).add_modifier(Modifier::BOLD),
            _ => Style::default(),
        }
    }

    /// lsplus size boundaries use raw bytes, independently of rounded display units.
    pub fn size_style(&self, bytes: Option<u64>) -> Style {
        if self.level == ColorLevel::NoColor {
            return Style::default();
        }
        match bytes {
            Some(1_073_741_824..) => self.named(AnsiColor::Red).add_modifier(Modifier::BOLD),
            Some(1_048_576..) => self.named(AnsiColor::Yellow),
            _ => Style::default(),
        }
    }

    /// lsplus permission roles, applied to spans instead of ANSI strings.
    pub fn permission_style(&self, value: char) -> Style {
        match value {
            'r' => self.named(AnsiColor::Green),
            'w' => self.named(AnsiColor::Yellow),
            'x' | 's' | 't' => self.named(AnsiColor::Red).add_modifier(Modifier::BOLD),
            '-' | 'S' | 'T' => Style::default().add_modifier(Modifier::DIM),
            _ => Style::default(),
        }
    }

    /// Keep lsplus age buckets; palette colours replace its optional RGB gradient.
    pub fn time_style(&self, time: Option<SystemTime>, now: SystemTime) -> Style {
        let Some(time) = time else {
            return self.style(Role::Disabled);
        };
        if self.level == ColorLevel::NoColor {
            return Style::default();
        }
        let Ok(age) = now.duration_since(time) else {
            return self.named(AnsiColor::Red).add_modifier(Modifier::BOLD);
        };
        if self.level == ColorLevel::Ansi16 {
            let style = self.named(AnsiColor::Yellow);
            return if age < DAY {
                style.add_modifier(Modifier::BOLD)
            } else if age < YEAR {
                style
            } else {
                style.add_modifier(Modifier::DIM)
            };
        }
        let index = if age < DAY {
            222
        } else if age < WEEK {
            221
        } else if age < MONTH {
            178
        } else if age < YEAR {
            136
        } else {
            130
        };
        let style = resolve_ansi256(index, self.level).map_or_else(Style::default, |color| {
            Style::default().fg(map_color(color))
        });
        if age < DAY {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }
}
