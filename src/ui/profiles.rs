use super::modal;
use crate::{
    app::App,
    color::{Role, Theme},
};
use ratatui::{Frame, layout::Rect, text::Line, widgets::Paragraph};

const HINT: &str = "Enter: active pane; Space: both panes; Esc: close";
const SAVE_HINT: &str = "Enter saves; Esc cancels";
const APPLY_BOTH: &str = "Apply selected to both: ";

pub(super) fn title(app: &App) -> &'static str {
    if app.profile_name.is_some() {
        "Save profile"
    } else {
        "View profiles"
    }
}

pub(super) fn layout(app: &App, bounds: Rect) -> modal::Layout<()> {
    let width = app
        .profile_names
        .iter()
        .map(|name| usize::from(modal::width(&super::clean(name))) + APPLY_BOTH.len())
        .max()
        .unwrap_or(0);
    let hint = if app.profile_name.is_some() {
        SAVE_HINT
    } else {
        HINT
    };
    let mut shell =
        super::view::measure(bounds, title(app), width, app.profile_names.len() + 2, hint);
    if app.profile_name.is_some() {
        let count = save_rows(app, usize::from(shell.body.width)).len();
        shell = super::view::measure(bounds, title(app), width, count, hint);
    }
    shell
}

pub(super) fn render(frame: &mut Frame, app: &App, shell: &modal::Layout<()>, theme: &Theme) {
    modal::render_measured(frame, shell, title(app), theme, Role::Header);
    let hint = if app.profile_name.is_some() {
        let lines: Vec<_> = save_rows(app, usize::from(shell.body.width))
            .into_iter()
            .map(|(role, text)| Line::styled(text, theme.style(role)))
            .collect();
        frame.render_widget(Paragraph::new(lines), shell.body);
        SAVE_HINT
    } else {
        let start = app
            .profile_menu
            .unwrap_or(0)
            .saturating_sub(usize::from(shell.body.height.saturating_sub(1)));
        render_rows(frame, app, shell.body, start, theme);
        HINT
    };
    frame.render_widget(
        Paragraph::new(super::text::wrap_value(hint, usize::from(shell.footer.width)).join("\n"))
            .style(theme.style(Role::Key)),
        shell.footer,
    );
}

fn save_rows(app: &App, width: usize) -> Vec<(Role, String)> {
    let mut rows: Vec<_> = super::text::wrap_value(
        &format!(
            "Name: {}",
            super::clean(app.profile_name.as_deref().unwrap_or_default())
        ),
        width,
    )
    .into_iter()
    .map(|text| (Role::Value, text))
    .collect();
    if let Some(error) = &app.profile_error {
        rows.extend(
            super::text::wrap_words(error, width)
                .into_iter()
                .map(|text| (Role::Error, text)),
        );
    }
    rows
}

fn render_rows(frame: &mut Frame, app: &App, area: Rect, start: usize, theme: &Theme) {
    let mut labels = app.profile_names.clone();
    labels.push(format!(
        "{APPLY_BOTH}{}",
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
                modal::truncate(&super::clean(&label), area.width),
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
