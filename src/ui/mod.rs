//! Render cached state only. No filesystem calls, subprocesses or worker submission.
mod diagnostics;
mod footer;
mod help;
pub use footer::FooterControl;
pub use help::{HelpLayout, context::ContextLayout};
mod layout;
mod listing;
mod measure;
mod metadata;
mod notifications;
mod status;
pub use notifications::{HistoryLayout, Toast};
pub mod modal;
mod operations;
pub(crate) use operations::animate_preview;
pub use operations::{OperationControl, OperationTarget};
mod path_title;
pub use metadata::MetadataControl;
mod pattern;
mod policy;
mod profiles;
pub use pattern::{PatternControl, PatternLayout};
mod summary;
mod text;
mod view;
use crate::{app::App, color::Theme};
pub use layout::{Geometry, geometry};
pub use measure::Measurements;
use ratatui::{Frame, layout::Rect};

fn status_text(app: &App, status: &crate::app::Status, width: u16) -> String {
    match status {
        crate::app::Status::Welcome => format!(
            "Local files — {} for help, {} for jobs",
            app.shortcut(crate::input::Action::Help, "?"),
            app.shortcut_hint(crate::input::Action::Jobs, "F9")
        ),
        crate::app::Status::Text(text) | crate::app::Status::Notice { text, .. } => clean(text),
        crate::app::Status::JobsDetails(text) => details_text(app, text, width),
    }
}

/// Keep the explicit details route visible after clipping untrusted prose.
fn details_text(app: &App, summary: &str, width: u16) -> String {
    let details = format!("{} details", app.shortcut(crate::input::Action::Jobs, "F9"));
    let suffix = format!(" — {details}");
    if width < modal::width(&suffix) {
        return modal::truncate(&details, width);
    }
    format!(
        "{}{suffix}",
        modal::truncate(&clean(summary), width.saturating_sub(modal::width(&suffix)))
    )
}

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
    status::render(frame, app, geometry.status, theme);
    footer::render(frame, geometry.footer, &geometry.footer_controls, theme);
    if app.popup {
        metadata::render(frame, app, overlay_bounds(app, frame.area()), theme, true);
    }
    if let Some(layout) = &geometry.help {
        help::render(frame, app, theme, layout);
    }
    if let Some(shell) = &geometry.menu_layout {
        if app.view_menu.is_some() {
            view::render(frame, app, shell, theme);
        } else {
            profiles::render(frame, app, shell, theme);
        }
    }
    if let Some(lines) = diagnostics {
        diagnostics::render(frame, app, theme, &mut geometry, lines);
    }
    pattern::render(frame, app, theme, geometry.pattern.as_ref());
    operations::render(frame, app, theme, &geometry);
    help::context::render(frame, app, theme, geometry.context_help.as_ref());
    notifications::render(frame, app, &geometry, theme);
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

fn overlay_bounds(app: &App, area: Rect) -> Rect {
    Rect {
        height: area
            .height
            .saturating_sub(status::rows(app, area).saturating_sub(1)),
        ..area
    }
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
