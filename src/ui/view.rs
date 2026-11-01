use super::modal;
use crate::{
    app::App,
    color::{Role, Theme},
    input::view,
};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    widgets::{Cell, Paragraph, Row, Table},
};

const TITLE: &str = "View options";
const HINT: &str = "Up/Down selects; Space/Enter changes; Esc closes";

pub(super) fn layout(app: &App, bounds: Rect) -> modal::Layout<()> {
    let rows = view::rows(app, app.metadata_presentation);
    let width = rows
        .iter()
        .map(|(label, value)| {
            usize::from(modal::width(label)) + usize::from(modal::width(value)) + 1
        })
        .max()
        .unwrap_or(0);
    measure(bounds, TITLE, width, rows.len(), HINT)
}

// Both menus reserve wrapped hints without displacing their last visible row.
pub(super) fn measure(
    bounds: Rect,
    title: &str,
    content_width: usize,
    count: usize,
    hint: &str,
) -> modal::Layout<()> {
    let mut spec = modal::Spec {
        title,
        help: true,
        content_width: content_width.max(usize::from(modal::width(hint))),
        pinned_rows: 0,
        footer_rows: 0,
        actions: &[],
    };
    let initial = spec.measure(bounds, |_| count);
    spec.footer_rows = (super::text::wrap_value(hint, usize::from(initial.body.width))
        .len()
        .min(usize::from(initial.body.height.saturating_sub(1)))) as u16;
    let mut shell = menu_shell(&spec, bounds, initial.outer.width, count);
    while spec.footer_rows > 0 && shell.body.height == 0 {
        spec.footer_rows -= 1;
        shell = menu_shell(&spec, bounds, initial.outer.width, count);
    }
    shell
}

fn menu_shell(
    spec: &modal::Spec<'_, ()>,
    bounds: Rect,
    width: u16,
    count: usize,
) -> modal::Layout<()> {
    let available = Rect {
        height: bounds.height.saturating_sub(2),
        ..bounds
    };
    let height = count
        .saturating_add(usize::from(spec.footer_rows) + 4)
        .min(usize::from(available.height)) as u16;
    modal::Layout::viewport(spec, super::centered(available, width, height))
}

pub(super) fn render(frame: &mut Frame, app: &App, shell: &modal::Layout<()>, theme: &Theme) {
    let index = app.view_menu.expect("menu geometry");
    let start = index.saturating_sub(usize::from(shell.body.height.saturating_sub(1)));
    let rows = view::rows(app, app.metadata_presentation)
        .into_iter()
        .enumerate()
        .skip(start)
        .map(|(row, (label, value))| {
            let style = theme.style(if row == index {
                Role::Cursor
            } else {
                Role::Normal
            });
            Row::new([
                Cell::from(label).style(theme.style(Role::Label).patch(style)),
                Cell::from(value).style(style),
            ])
        });
    modal::render_measured(frame, shell, TITLE, theme, Role::Header);
    frame.render_widget(
        Table::new(
            rows,
            [Constraint::Percentage(60), Constraint::Percentage(40)],
        )
        .column_spacing(1),
        shell.body,
    );
    frame.render_widget(
        Paragraph::new(super::text::wrap_value(HINT, usize::from(shell.footer.width)).join("\n"))
            .style(theme.style(Role::Key)),
        shell.footer,
    );
}
