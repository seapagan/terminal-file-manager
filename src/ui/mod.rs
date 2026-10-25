//! Render cached state only. No filesystem calls, subprocesses or worker submission.
mod diagnostics;
mod footer;
mod help;
pub use footer::FooterControl;
mod layout;
mod listing;
mod measure;
mod metadata;
pub mod modal;
mod operations;
pub use operations::{OperationControl, OperationTarget};
mod path_title;
pub use metadata::MetadataControl;
mod policy;
mod profiles;
pub(crate) use profiles::hit as profile_hit;
mod summary;
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
    let mut output = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_control()
            || matches!(c, '\u{200e}' | '\u{200f}' | '\u{61c}' | '\u{2028}' | '\u{2029}'
                    | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            output.extend(c.escape_default());
        } else {
            output.push(c);
        }
    }
    output
}

/// Draw a frame using the same geometry used by mouse hit testing.
pub fn render(frame: &mut Frame, app: &App, theme: &Theme) -> Geometry {
    let mut geometry = geometry(app, frame.area());
    for (id, area) in geometry.panes.iter().enumerate() {
        if area.is_some() {
            listing::render(frame, app, id, &geometry, theme);
        }
    }
    let diagnostics = app
        .diagnostics
        .then(|| diagnostics::report(frame.buffer_mut(), app, &geometry, theme));
    if let Some(area) = geometry.sidebar {
        metadata::render(frame, app, area, theme, false);
    }
    frame.render_widget(
        Paragraph::new(clean(&app.status)).style(theme.style(Role::Status)),
        geometry.status,
    );
    footer::render(frame, geometry.footer, &geometry.footer_controls, theme);
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
    if let Some(lines) = diagnostics {
        diagnostics::render(frame, app, theme, &mut geometry, lines);
    }
    operations::render(frame, app, theme, &geometry);
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

#[cfg(test)]
mod clean_tests {
    #[test]
    fn terminal_controls_and_direction_overrides_are_escaped_without_changing_text() {
        assert_eq!(super::clean("a\u{1b}[31m\n"), "a\\u{1b}[31m\\n");
        assert_eq!(super::clean("\u{202e}x\u{2069}"), "\\u{202e}x\\u{2069}");
        assert_eq!(super::clean("normal λ界👩"), "normal λ界👩");
    }
}
