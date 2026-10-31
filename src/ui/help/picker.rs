use crate::{
    app::App,
    color::{Role, Theme},
    input::{HelpHit, help_hits},
    ui::{modal, text::wrap_value},
};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

/// Each visible result has one complete, measured two-line mouse region.
#[derive(Clone, Debug)]
pub struct PickerLayout {
    pub shell: modal::Layout<()>,
    pub query: Rect,
    pub hits: Vec<HelpHit>,
    pub controls: Vec<(Rect, usize)>,
}

pub(super) fn layout(app: &App, bounds: Rect) -> Option<PickerLayout> {
    let state = app.help_picker.as_ref()?;
    let hits = help_hits(&app.help_rows, &state.query);
    let shell = modal::Spec::<()> {
        title: "Jump to section or search commands",
        help: false,
        content_width: 62,
        pinned_rows: 1,
        footer_rows: 1,
        actions: &[],
    }
    .measure(bounds, |_| (hits.len().clamp(1, 7)) * 2);
    let selected = state.selected.min(hits.len().saturating_sub(1));
    let capacity = usize::from(shell.body.height / 2);
    let start = selected.saturating_sub(capacity.saturating_sub(1));
    let controls = (start..hits.len())
        .take(capacity)
        .enumerate()
        .map(|(row, index)| {
            (
                Rect::new(
                    shell.body.x,
                    shell.body.y + row as u16 * 2,
                    shell.body.width,
                    2,
                ),
                index,
            )
        })
        .collect();
    Some(PickerLayout {
        query: shell.pinned,
        shell,
        hits,
        controls,
    })
}

pub(super) fn render(frame: &mut Frame, app: &App, theme: &Theme, layout: &PickerLayout) {
    modal::render_shell(
        frame,
        layout.shell.outer,
        "Jump to section or search commands",
        theme,
    );
    let state = app.help_picker.as_ref().expect("visible picker");
    let text = format!("> {}", crate::ui::clean(&state.query));
    let text = wrap_value(&text, usize::from(layout.query.width))
        .pop()
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(text).style(theme.style(Role::Cursor)),
        layout.query,
    );
    if layout.query.width > 0 && layout.query.height > 0 {
        frame.set_cursor_position((
            layout.query.x
                + modal::width(&state.query)
                    .saturating_add(2)
                    .min(layout.query.width - 1),
            layout.query.y,
        ));
    }
    if layout.hits.is_empty() {
        frame.render_widget(
            Paragraph::new("No matching commands").style(theme.style(Role::Value)),
            layout.shell.body,
        );
    }
    render_hits(frame, app, theme, layout);
    let hint = format!("↑/↓ Enter/click · Esc back · {} hits", layout.hits.len());
    frame.render_widget(
        Paragraph::new(modal::truncate(&hint, layout.shell.footer.width))
            .style(theme.style(Role::Key)),
        layout.shell.footer,
    );
}

fn render_hits(frame: &mut Frame, app: &App, theme: &Theme, layout: &PickerLayout) {
    let state = app.help_picker.as_ref().expect("visible picker");
    for (area, index) in &layout.controls {
        let hit = &layout.hits[*index];
        let selected = *index == state.selected.min(layout.hits.len().saturating_sub(1));
        let title = format!("{} {}", if selected { ">" } else { " " }, hit.title);
        let subtitle = if hit.keys.is_empty() {
            "Section".into()
        } else {
            format!("{} · {}", hit.keys, hit.section.title())
        };
        let lines = [
            modal::truncate(&title, area.width),
            modal::truncate(&subtitle, area.width),
        ];
        frame.render_widget(
            Paragraph::new(lines.join("\n")).style(theme.style(if selected {
                Role::Cursor
            } else {
                Role::Value
            })),
            *area,
        );
    }
}
