//! Render cached state only. No filesystem calls, subprocesses or worker submission.
mod layout;
mod listing;
mod metadata;
use crate::{
    app::App,
    color::{Role, Theme},
};
pub use layout::{Geometry, geometry};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

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
        metadata::render(frame, app, area, theme);
    }
    frame.render_widget(Paragraph::new(clean(&app.status)), geometry.status);
    frame.render_widget(
        Paragraph::new("? Help  ^P Panes  Alt-I Info  / Filter  q Quit | F3–F8 unavailable")
            .style(theme.style(Role::Disabled)),
        geometry.footer,
    );
    if app.popup {
        metadata::render(frame, app, overlay(frame.area()), theme);
    }
    if app.help {
        render_help(frame, theme, app.overlay_scroll);
    }
    geometry
}

pub(super) fn overlay(area: Rect) -> Rect {
    let margin = if area.width >= 50 && area.height >= 10 {
        2
    } else {
        0
    };
    Rect::new(
        area.x + margin,
        area.y + margin,
        area.width.saturating_sub(margin * 2),
        area.height.saturating_sub(margin * 2),
    )
}

fn render_help(frame: &mut Frame, theme: &Theme, scroll: u16) {
    let area = overlay(frame.area());
    frame.render_widget(Clear, area);
    let text = "READ-ONLY LOCAL BROWSER\n\n↑/↓ or j/k: cursor   Home/End/PgUp/PgDn: move\nEnter: enter directory; files never launch\nBackspace/Alt-Up: parent   Alt-Left/Right: history\nCtrl-P: one/two panes   Tab: focus other visible pane\nAlt-I: sidebar   Alt-Enter: metadata popup\nSpace: mark   Insert: mark and advance\nCtrl-A: select visible   Ctrl-U: clear visible selection\n/: fuzzy filter   Enter: accept   Esc: cancel/clear\nh: hidden files   i: ignore dim/hide/off   r: refresh\ns: next sort   v: reverse   g: directory grouping\nc: directory totals   Esc: cancel totals/close overlay\nMouse: click focus/cursor, double-click enter, wheel per pane\nCtrl-C/q/F10: quit without changing the shell directory\n\nF3/viewer and F4/editor: unavailable until M6\nF5/copy F6/move F7/mkdir F8/delete: unavailable until M2\nCtrl-L/z/Z/bookmarks/shell handoff: unavailable until M5\n\nBindings can be changed in config.toml. Esc/? closes help.";
    frame.render_widget(
        Paragraph::new(text)
            .scroll((scroll, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help — Esc closes"),
            )
            .style(theme.style(Role::Normal))
            .wrap(Wrap { trim: false }),
        area,
    );
}
