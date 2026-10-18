use crate::{
    app::App,
    color::{Role, Theme},
    input::view,
};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    widgets::{Block, Borders, Cell, Clear, Row, Table},
};

pub(super) fn area(bounds: Rect, count: usize) -> Rect {
    super::centered(
        bounds,
        bounds.width.saturating_sub(2).min(64),
        bounds
            .height
            .saturating_sub(2)
            .min(u16::try_from(count.saturating_add(2)).unwrap_or(u16::MAX)),
    )
}

pub(super) fn render(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let index = app.view_menu.expect("menu geometry");
    let rows = view::rows(app, app.metadata_presentation);
    menu(
        frame,
        area,
        rows,
        index,
        theme,
        "View options — Space/Enter changes; Esc closes",
    );
}

pub(super) fn menu(
    frame: &mut Frame,
    area: Rect,
    rows: Vec<(String, String)>,
    index: usize,
    theme: &Theme,
    title: &str,
) {
    let start = index.saturating_sub(usize::from(area.height.saturating_sub(3)));
    let rows = rows
        .into_iter()
        .enumerate()
        .skip(start)
        .map(|(row, (label, value))| {
            let style = if row == index {
                theme.style(Role::Cursor)
            } else {
                theme.style(Role::Normal)
            };
            Row::new([
                Cell::from(label).style(theme.style(Role::Label).patch(style)),
                Cell::from(value).style(style),
            ])
        });
    frame.render_widget(Clear, area);
    frame.render_widget(
        Table::new(
            rows,
            [Constraint::Percentage(60), Constraint::Percentage(40)],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(theme.style(Role::Header)),
        )
        .column_spacing(1),
        area,
    );
}
