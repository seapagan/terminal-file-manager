//! Renderer-neutral adapter adapted from Labeldeck's edit/color.rs and UI theme.
mod evidence;
mod listing;
use colored_text::{AnsiColor, ColorLevel, ResolvedColor, resolve_hex, resolve_named, resolve_rgb};
use ratatui::style::{Color, Modifier, Style};

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
}

impl Theme {
    /// Capability detection and environment precedence belong to colored_text.
    pub fn new(level: ColorLevel) -> Self {
        Self {
            level,
            evidence: Vec::new(),
        }
    }

    /// Detect stdout capability once, retaining the startup evidence for diagnostics.
    pub fn for_stdout() -> Self {
        let mut theme = Self::new(colored_text::ColorizeConfig::color_level(
            colored_text::RenderTarget::Stdout,
        ));
        theme.evidence = evidence::startup();
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
