//! Listing context composes the shared palette before cursor decoration.
use super::{Role, Theme};
use crate::fs::Entry;
use ratatui::style::Style;

impl Theme {
    /// Mute inactive browser foregrounds without changing any background colour.
    pub(crate) fn dim_listing_style(&self, style: Style) -> Style {
        use colored_text::ColorLevel;
        use ratatui::style::{Color, Modifier};
        let foreground = match self.level {
            ColorLevel::NoColor => Color::Reset,
            ColorLevel::Ansi16 => Color::DarkGray,
            ColorLevel::Ansi256 => Color::Indexed(242),
            ColorLevel::TrueColor => Color::Rgb(110, 110, 110),
        };
        let style = style
            .fg(foreground)
            .remove_modifier(Modifier::BOLD | Modifier::DIM);
        if self.level == ColorLevel::NoColor {
            style.add_modifier(Modifier::DIM)
        } else {
            style
        }
    }
    /// System filenames use the shared red accent; type indicators keep their style.
    pub fn name_style(&self, entry: &Entry) -> Style {
        let style = self.entry_style(entry);
        if entry.is_system() {
            style.patch(self.style(Role::System))
        } else {
            style
        }
    }

    /// Keep faint effective on VTE, which ignores it on direct RGB foregrounds.
    /// Cursor styling is applied afterwards and retains precedence.
    pub fn ignored_name_style(&self, entry: &Entry, active: bool) -> Style {
        let style = self.name_style(entry).patch(self.style(Role::Ignored));
        let inherited = active
            .then(|| self.style(Role::ActivePane))
            .and_then(|s| s.fg);
        self.faint_style(style, inherited)
    }

    /// VTE needs indexed cyan for faint hidden markers; blank cells keep their style.
    pub(crate) fn hidden_indicator_style(&self, hidden: bool) -> Style {
        let style = self.style(Role::Hidden);
        if hidden {
            self.faint_style(style, None)
        } else {
            style
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use colored_text::ColorLevel;
    use ratatui::style::{Color, Modifier};

    #[test]
    fn browser_muting_changes_only_foreground_intensity_and_preserves_backgrounds() {
        for level in [
            ColorLevel::NoColor,
            ColorLevel::Ansi16,
            ColorLevel::Ansi256,
            ColorLevel::TrueColor,
        ] {
            for background in [Color::Reset, Color::Indexed(123), Color::Rgb(17, 28, 39)] {
                let original = Style::default()
                    .fg(Color::Red)
                    .bg(background)
                    .add_modifier(Modifier::BOLD | Modifier::ITALIC | Modifier::REVERSED);
                let muted = Theme::new(level).dim_listing_style(original);
                assert_eq!(muted.bg, original.bg);
                assert!(
                    muted
                        .add_modifier
                        .contains(Modifier::ITALIC | Modifier::REVERSED)
                );
                assert!(!muted.add_modifier.contains(Modifier::BOLD));
                assert_eq!(
                    muted.add_modifier.contains(Modifier::DIM),
                    level == ColorLevel::NoColor
                );
            }
        }
    }
}
