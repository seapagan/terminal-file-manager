use crate::{
    app::App,
    color::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    text::Line,
    widgets::{Block, Borders, Clear, Padding, Paragraph},
};

const HINT: &str = "Enter: active pane; Space: both…; Esc: close";

fn block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1))
}

pub(super) fn area(app: &App, bounds: Rect) -> Rect {
    let width = app
        .profile_names
        .iter()
        .map(|s| Line::raw(super::clean(s)).width())
        .chain([HINT.len()])
        .max()
        .unwrap_or(0)
        .saturating_add(4);
    super::centered(
        bounds,
        u16::try_from(width)
            .unwrap_or(u16::MAX)
            .min(bounds.width.saturating_sub(2)),
        u16::try_from(app.profile_names.len().saturating_add(5))
            .unwrap_or(u16::MAX)
            .min(bounds.height.saturating_sub(2)),
    )
}

fn viewport(app: &App, area: Rect) -> (Rect, Rect, usize) {
    let inner = block().inner(area);
    let hints = super::text::wrap_value(HINT, usize::from(inner.width));
    let footer_height = (hints.len() as u16).min(inner.height.saturating_sub(1));
    let rows = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(footer_height),
    );
    let footer = Rect::new(inner.x, rows.bottom(), inner.width, footer_height);
    let start = app
        .profile_menu
        .unwrap_or(0)
        .saturating_sub(usize::from(rows.height.saturating_sub(1)));
    (rows, footer, start)
}

pub(crate) fn hit(app: &App, area: Rect, point: Position) -> Option<usize> {
    let (rows, _, start) = viewport(app, area);
    let index = start + usize::from(point.y.saturating_sub(rows.y));
    (rows.contains(point) && index < app.profile_names.len() + 2).then_some(index)
}

fn fit(text: String, width: u16) -> String {
    let text = super::clean(&text);
    if Line::raw(&text).width() <= usize::from(width) {
        return text;
    }
    if width == 0 {
        return String::new();
    }
    format!(
        "{}…",
        super::text::wrap_value(&text, usize::from(width - 1))[0]
    )
}

pub(super) fn render(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let title = if app.profile_name.is_some() {
        "Save profile"
    } else {
        "View profiles"
    };
    frame.render_widget(Clear, area);
    frame.render_widget(
        block().title(title).border_style(theme.style(Role::Header)),
        area,
    );
    let (rows, footer, start) = viewport(app, area);
    if let Some(name) = &app.profile_name {
        let lines = super::text::wrap_value(
            &format!("Name: {}", super::clean(name)),
            usize::from(rows.width),
        );
        frame.render_widget(
            Paragraph::new(lines.join("\n")).style(theme.style(Role::Value)),
            rows,
        );
        frame.render_widget(
            Paragraph::new("Enter saves; Esc cancels").wrap(ratatui::widgets::Wrap { trim: false }),
            footer,
        );
        return;
    }
    render_rows(frame, app, rows, start, theme);
    let hints = super::text::wrap_value(HINT, usize::from(footer.width));
    frame.render_widget(
        Paragraph::new(hints.join("\n")).style(theme.style(Role::Key)),
        footer,
    );
}

fn render_rows(frame: &mut Frame, app: &App, area: Rect, start: usize, theme: &Theme) {
    let mut labels = app.profile_names.clone();
    labels.push(format!(
        "Apply selected to both: {}",
        app.profile_names
            .get(app.profile_choice)
            .map_or("", String::as_str)
    ));
    labels.push("Save active as…".into());
    let lines: Vec<_> = labels
        .into_iter()
        .enumerate()
        .skip(start)
        .take(usize::from(area.height))
        .map(|(index, label)| {
            Line::styled(
                fit(label, area.width),
                theme.style(if Some(index) == app.profile_menu {
                    Role::Cursor
                } else {
                    Role::Normal
                }),
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}
