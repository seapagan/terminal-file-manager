//! Renderer-neutral adapter adapted from Labeldeck's edit/color.rs and UI theme.
use colored_text::{AnsiColor, ColorLevel, ResolvedColor, resolve_hex, resolve_named};
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
}

impl Theme {
    /// Capability detection and environment precedence belong to colored_text.
    pub fn new(level: ColorLevel) -> Self {
        Self { level }
    }

    /// Resolve a configured hexadecimal accent without copying library internals.
    ///
    /// # Errors
    /// Returns the library's precise malformed-colour error.
    pub fn resolve(&self, hex: &str) -> Result<Option<Color>, colored_text::ColorInputError> {
        resolve_hex(hex, self.level).map(|color| color.map(map_color))
    }

    /// Focus, cursor and marks remain distinguishable in monochrome terminals.
    pub fn style(&self, role: Role) -> Style {
        let named = match role {
            Role::Header | Role::Link | Role::Label | Role::Key => Some(AnsiColor::Cyan),
            Role::Status => Some(AnsiColor::Green),
            Role::Error => Some(AnsiColor::Red),
            Role::Directory => Some(AnsiColor::Blue),
            Role::Executable => Some(AnsiColor::Green),
            Role::Warning => Some(AnsiColor::Yellow),
            Role::Marked => Some(AnsiColor::Magenta),
            _ => None,
        };
        let mut style = Style::default();
        if let Some(color) = named.and_then(|c| resolve_named(c, self.level)) {
            style = style.fg(map_color(color));
        }
        match role {
            Role::ActivePane => {
                // Use a paired terminal palette foreground/background, never an RGB theme.
                if let Some(bg) = resolve_named(AnsiColor::BrightBlack, self.level) {
                    style = style.bg(map_color(bg));
                }
                if let Some(fg) = resolve_named(AnsiColor::BrightWhite, self.level) {
                    style = style.fg(map_color(fg));
                }
                style
            }
            Role::Cursor => style
                .remove_modifier(Modifier::DIM)
                .add_modifier(Modifier::REVERSED | Modifier::BOLD),
            Role::InactiveCursor => style.add_modifier(Modifier::DIM | Modifier::BOLD),
            Role::Header | Role::Marked | Role::Label | Role::Key | Role::Error => {
                style.add_modifier(Modifier::BOLD)
            }
            Role::Ignored | Role::Disabled => style.add_modifier(Modifier::DIM),
            _ => style,
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
