//! Renderer-neutral adapter adapted from Labeldeck's edit/color.rs and UI theme.
mod evidence;
mod listing;
use crate::{
    fs::{Entry, EntryKind},
    fuzzy_time::{DAY, MONTH, WEEK, YEAR},
};
use colored_text::{
    AnsiColor, ColorLevel, ResolvedColor, resolve_ansi256, resolve_hex, resolve_named, resolve_rgb,
};
use ratatui::style::{Color, Modifier, Style};
use std::time::SystemTime;

/// Semantic styles never embed ANSI escape strings in widgets.
#[derive(Clone, Copy)]
pub enum Role {
    Normal,
    Header,
    Directory,
    Executable,
    Link,
    Warning,
    Ignored,
    Hidden,
    Cursor,
    InactiveCursor,
    ActivePane,
    Label,
    Value,
    Key,
    Status,
    Error,
    Marked,
    Disabled,
}

/// Terminal-native palette with capability-aware colour resolution.
pub struct Theme {
    pub level: ColorLevel,
    pub(crate) evidence: Vec<String>,
    vte: bool,
}

impl Theme {
    /// Capability detection and environment precedence belong to colored_text.
    pub fn new(level: ColorLevel) -> Self {
        Self::with_vte_version(level, None)
    }

    /// Resolve an explicitly supplied terminal identity without reading the environment.
    pub fn with_vte_version(level: ColorLevel, version: Option<&str>) -> Self {
        Self {
            level,
            evidence: Vec::new(),
            vte: version.is_some_and(|value| {
                !value.is_empty()
                    && value.bytes().all(|byte| byte.is_ascii_digit())
                    && value.parse::<u32>().is_ok_and(|version| version > 0)
            }),
        }
    }

    /// Detect stdout capability once, retaining the startup evidence for diagnostics.
    pub fn for_stdout() -> Self {
        let version = std::env::var_os("VTE_VERSION");
        let mut theme = Self::with_vte_version(
            colored_text::ColorizeConfig::color_level(colored_text::RenderTarget::Stdout),
            version.as_deref().and_then(std::ffi::OsStr::to_str),
        );
        theme.evidence = evidence::startup(version.as_deref());
        theme
    }

    /// Resolve a configured hexadecimal accent without copying library internals.
    ///
    /// # Errors
    /// Returns the library's precise malformed-colour error.
    pub fn resolve(&self, hex: &str) -> Result<Option<Color>, colored_text::ColorInputError> {
        resolve_hex(hex, self.level).map(|color| color.map(map_color))
    }

    // Mid-range accents leave the unknown terminal foreground/background alone.
    fn accent(&self, named: AnsiColor) -> Option<Color> {
        let rgb = match named {
            AnsiColor::Red => (190, 65, 70),
            AnsiColor::Green => (50, 145, 90),
            AnsiColor::Yellow => (165, 125, 35),
            AnsiColor::Blue => (70, 120, 185),
            AnsiColor::Magenta => (155, 90, 160),
            AnsiColor::Cyan => (35, 140, 150),
            _ => return resolve_named(named, self.level).map(map_color),
        };
        if self.level >= ColorLevel::Ansi256 {
            resolve_rgb(rgb.0, rgb.1, rgb.2, self.level).map(map_color)
        } else {
            resolve_named(named, self.level).map(map_color)
        }
    }

    /// Focus, cursor and marks remain distinguishable in monochrome terminals.
    pub fn style(&self, role: Role) -> Style {
        let named = match role {
            Role::Header | Role::Link | Role::Label | Role::Key | Role::Hidden => {
                Some(AnsiColor::Cyan)
            }
            Role::Status => Some(AnsiColor::Green),
            Role::Error => Some(AnsiColor::Red),
            Role::Directory => Some(AnsiColor::Blue),
            Role::Executable => Some(AnsiColor::Green),
            Role::Warning => Some(AnsiColor::Yellow),
            Role::Marked => Some(AnsiColor::Magenta),
            _ => None,
        };
        let mut style = Style::default();
        if let Some(color) = named.and_then(|c| self.accent(c)) {
            style = style.fg(color);
        }
        match role {
            Role::ActivePane if self.level >= ColorLevel::Ansi256 => {
                self.pair((220, 225, 230), (22, 28, 36))
            }
            Role::Cursor | Role::InactiveCursor => self.cursor_style(matches!(role, Role::Cursor)),
            Role::Header | Role::Marked | Role::Label | Role::Key | Role::Error => {
                style.add_modifier(Modifier::BOLD)
            }
            Role::Ignored | Role::Hidden | Role::Disabled => style.add_modifier(Modifier::DIM),
            _ => style,
        }
    }

    fn pair(&self, fg: (u8, u8, u8), bg: (u8, u8, u8)) -> Style {
        let resolve = |(r, g, b)| resolve_rgb(r, g, b, self.level).map(map_color);
        Style {
            fg: resolve(fg),
            bg: resolve(bg),
            ..Style::default()
        }
    }

    fn cursor_style(&self, active: bool) -> Style {
        let style = match self.level {
            ColorLevel::NoColor => Style::default()
                .fg(Color::Reset)
                .bg(Color::Reset)
                .add_modifier(Modifier::REVERSED),
            ColorLevel::Ansi16 => {
                let (fg, bg) = if active {
                    (AnsiColor::Black, AnsiColor::Cyan)
                } else {
                    (AnsiColor::BrightWhite, AnsiColor::Blue)
                };
                let style = Style {
                    fg: resolve_named(fg, self.level).map(map_color),
                    bg: resolve_named(bg, self.level).map(map_color),
                    ..Style::default()
                };
                style.remove_modifier(Modifier::REVERSED)
            }
            _ => {
                let (fg, bg) = if active {
                    ((245, 248, 250), (35, 85, 125))
                } else {
                    ((184, 192, 202), (41, 47, 56))
                };
                self.pair(fg, bg).remove_modifier(Modifier::REVERSED)
            }
        }
        .remove_modifier(Modifier::DIM | Modifier::BOLD);
        if active {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }

    // VTE ignores faint on direct RGB; only the requesting name/marker opts in.
    fn faint_style(&self, mut style: Style, inherited: Option<Color>) -> Style {
        if self.vte
            && self.level == ColorLevel::TrueColor
            && let Some(Color::Rgb(r, g, b)) = style.fg.or(inherited)
        {
            style.fg = resolve_rgb(r, g, b, ColorLevel::Ansi256).map(map_color);
        }
        style
    }

    // lsplus file.rs/render.rs classification (MIT, Grant Ramsay).
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

/// Explicit named/indexed/RGB mapping preserves Ratatui's cell semantics.
pub fn map_color(resolved: ResolvedColor) -> Color {
    match resolved {
        ResolvedColor::Ansi256(n) => Color::Indexed(n),
        ResolvedColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
        ResolvedColor::Named(named) => match named {
            AnsiColor::Black => Color::Black,
            AnsiColor::Red => Color::Red,
            AnsiColor::Green => Color::Green,
            AnsiColor::Yellow => Color::Yellow,
            AnsiColor::Blue => Color::Blue,
            AnsiColor::Magenta => Color::Magenta,
            AnsiColor::Cyan => Color::Cyan,
            AnsiColor::White => Color::Gray,
            AnsiColor::BrightBlack => Color::DarkGray,
            AnsiColor::BrightRed => Color::LightRed,
            AnsiColor::BrightGreen => Color::LightGreen,
            AnsiColor::BrightYellow => Color::LightYellow,
            AnsiColor::BrightBlue => Color::LightBlue,
            AnsiColor::BrightMagenta => Color::LightMagenta,
            AnsiColor::BrightCyan => Color::LightCyan,
            AnsiColor::BrightWhite => Color::White,
        },
    }
}
