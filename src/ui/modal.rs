//! Measured overlay shell shared by cached operation and metadata presentation.
pub mod actions;
mod content;
pub use content::Row;
mod files;
mod layout;
#[cfg(test)]
mod tests;
use crate::color::{Role, Theme};
pub use files::{common_root, entry_row, file_row, file_size, truncate};
pub use layout::{ActionRegion, Layout};
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Content measurements exclude borders and padding. Rows may wrap at `body_width`.
pub struct Spec<'a, T> {
    /// Sanitized display title.
    pub title: &'a str,
    /// Intrinsic content width in display cells.
    pub content_width: usize,
    /// Fixed rows above the scrollable body (for example a Jobs selector).
    pub pinned_rows: u16,
    /// Fixed warning/error rows above the actions.
    pub footer_rows: u16,
    /// Typed actions; authorization remains with the caller.
    pub actions: &'a [actions::Action<T>],
}

impl<T: Clone> Spec<'_, T> {
    /// Measure once, then use the returned layout for drawing and hit testing.
    pub fn measure(&self, terminal: Rect, body_rows: impl FnOnce(u16) -> usize) -> Layout<T> {
        Layout::new(self, terminal, body_rows)
    }
}

/// Display-cell width with saturation at Ratatui's coordinate limit.
pub fn width(text: &str) -> u16 {
    Line::raw(text).width().min(usize::from(u16::MAX)) as u16
}

/// Clear and draw only the measured shell. Normal successive frames restore its backdrop.
pub fn render_shell(frame: &mut Frame, area: Rect, title: &str, theme: &Theme) {
    render_shell_with_title_role(frame, area, title, theme, Role::Header);
}

/// Draw a semantic title without changing the border or content geometry.
pub fn render_shell_with_title_role(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    theme: &Theme,
    role: Role,
) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(Role::Header))
            .style(theme.style(Role::Normal)),
        area,
    );
    if area.height > 0 && area.width > 3 {
        let shown = truncate(title, area.width - 3);
        let title_area = Rect::new(area.x + 2, area.y, width(&shown), 1);
        frame.render_widget(Paragraph::new(shown).style(theme.style(role)), title_area);
    }
}
