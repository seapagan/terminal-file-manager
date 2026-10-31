use crate::{
    app::App,
    color::{Role, Theme},
    input::{HelpHit, HelpTarget, help_hits},
    ui::{modal, text::wrap_value},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

/// Complete result regions exclude the query and its separator.
#[derive(Clone, Debug)]
pub struct PickerLayout {
    pub shell: modal::Layout<()>,
    pub query: Rect,
    pub separator: Rect,
    pub hits: Vec<HelpHit>,
    pub controls: Vec<(Rect, usize)>,
}

pub(super) fn layout(app: &App, bounds: Rect) -> Option<PickerLayout> {
    let state = app.help_picker.as_ref()?;
    let hits = help_hits(&app.help_rows, &state.query);
    let rows = hits
        .iter()
        .take(7)
        .map(|hit| usize::from(height(hit)))
        .sum::<usize>()
        .max(1);
    let shell = modal::Spec::<()> {
        title: "Jump to section or search commands",
        help: false,
        content_width: 62,
        pinned_rows: 2,
        footer_rows: 1,
        actions: &[],
    }
    .measure(bounds, |_| rows);
    let query = Rect::new(
        shell.pinned.x,
        shell.pinned.y,
        shell.pinned.width,
        shell.pinned.height.min(1),
    );
    let separator = Rect::new(
        query.x,
        query.bottom(),
        query.width,
        shell.pinned.height.saturating_sub(1),
    );
    let controls = controls(&hits, state.selected, shell.body);
    Some(PickerLayout {
        shell,
        query,
        separator,
        hits,
        controls,
    })
}

fn height(hit: &HelpHit) -> u16 {
    match hit.target {
        HelpTarget::Section(_) => 1,
        HelpTarget::Action(_) => 2,
    }
}

fn controls(hits: &[HelpHit], selected: usize, body: Rect) -> Vec<(Rect, usize)> {
    let Some(hit) = hits.get(selected.min(hits.len().saturating_sub(1))) else {
        return Vec::new();
    };
    let mut start = selected.min(hits.len() - 1);
    let mut used = height(hit);
    let mut count = 1;
    while start > 0 && count < 7 && used + height(&hits[start - 1]) <= body.height {
        start -= 1;
        used += height(&hits[start]);
        count += 1;
    }
    let mut y = body.y;
    let mut controls = Vec::new();
    for (index, hit) in hits.iter().enumerate().skip(start).take(7) {
        let height = height(hit);
        if y.saturating_add(height) > body.bottom() {
            break;
        }
        controls.push((Rect::new(body.x, y, body.width, height), index));
        y = y.saturating_add(height);
    }
    controls
}

pub(super) fn render(frame: &mut Frame, app: &App, theme: &Theme, layout: &PickerLayout) {
    modal::render_shell(
        frame,
        layout.shell.outer,
        "Jump to section or search commands",
        theme,
    );
    render_query(frame, app, theme, layout.query);
    frame.render_widget(
        Paragraph::new("─".repeat(usize::from(layout.separator.width)))
            .style(theme.style(Role::Header)),
        layout.separator,
    );
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

fn render_query(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let state = app.help_picker.as_ref().expect("visible picker");
    let label = modal::truncate(if area.width >= 10 { "Search: " } else { "/ " }, area.width);
    let label_width = modal::width(&label);
    let available = area.width.saturating_sub(label_width + 1);
    let text = if available == 0 {
        String::new()
    } else {
        wrap_value(&crate::ui::clean(&state.query), usize::from(available))
            .pop()
            .unwrap_or_default()
    };
    let cursor = label_width + modal::width(&text);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(label, theme.style(Role::Label)),
            Span::styled(text, theme.style(Role::Value)),
        ])),
        area,
    );
    if area.width > 0 && area.height > 0 {
        frame.set_cursor_position((area.x + cursor.min(area.width - 1), area.y));
    }
}

fn render_hits(frame: &mut Frame, app: &App, theme: &Theme, layout: &PickerLayout) {
    let state = app.help_picker.as_ref().expect("visible picker");
    for (area, index) in &layout.controls {
        let hit = &layout.hits[*index];
        let selected = *index == state.selected.min(layout.hits.len().saturating_sub(1));
        let style = theme.style(if selected { Role::Cursor } else { Role::Value });
        let title = format!("{} {}", if selected { ">" } else { " " }, hit.title);
        let mut lines = vec![Line::raw(modal::truncate(&title, area.width))];
        if area.height == 2 {
            lines.push(details(hit, area.width, theme, selected));
        }
        frame.render_widget(Paragraph::new(lines).style(style), *area);
    }
}

fn details(hit: &HelpHit, width: u16, theme: &Theme, selected: bool) -> Line<'static> {
    let keys = if hit.keys.is_empty() {
        "Unbound"
    } else {
        &hit.keys
    };
    let keys = modal::truncate(keys, width.saturating_sub(2));
    let remaining = width.saturating_sub(modal::width(&keys) + 2);
    let section = if remaining >= 4 {
        format!(" · {}", modal::truncate(hit.section.title(), remaining - 3))
    } else {
        String::new()
    };
    let mut key_style = theme.style(if hit.keys.is_empty() {
        Role::Value
    } else {
        Role::Key
    });
    if selected {
        key_style = key_style.patch(theme.style(Role::Cursor));
    }
    Line::from(vec![
        Span::raw("  "),
        Span::styled(keys, key_style),
        Span::raw(section),
    ])
}
