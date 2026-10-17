//! Render cached state only. No filesystem calls, subprocesses or worker submission.
mod help;
mod layout;
mod listing;
mod measure;
mod metadata;
mod profiles;
mod text;
mod view;
use crate::{
    app::App,
    color::{Role, Theme},
};
pub use layout::{Geometry, geometry};
pub use measure::Measurements;
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

/// Escape terminal controls and bidi formatting in every untrusted display string.
pub fn clean(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            if c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}

/// Draw a frame using the same geometry used by mouse hit testing.
pub fn render(frame: &mut Frame, app: &App, theme: &Theme) -> Geometry {
    let geometry = geometry(app, frame.area());
    for (id, area) in geometry.panes.iter().enumerate() {
        if let Some(area) = area {
            listing::render(frame, app, id, *area, theme);
        }
    }
    if let Some(area) = geometry.sidebar {
        metadata::render(frame, app, area, theme, false);
    }
    frame.render_widget(
        Paragraph::new(clean(&app.status)).style(theme.style(Role::Status)),
        geometry.status,
    );
    frame.render_widget(
        Paragraph::new("? Help  ^P Panes  Alt-I Info  / Filter  q Quit | F3–F8 unavailable")
            .style(theme.style(Role::Disabled)),
        geometry.footer,
    );
    if app.popup {
        metadata::render(frame, app, frame.area(), theme, true);
    }
    if app.help {
        help::render(frame, app, theme);
    }
    if let Some(area) = geometry.menu {
        if app.view_menu.is_some() {
            view::render(frame, app, area, theme);
        } else {
            profiles::render(frame, app, area, theme);
        }
    }
    geometry
}

pub(super) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
