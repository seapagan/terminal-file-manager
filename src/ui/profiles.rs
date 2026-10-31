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
    if let Some(name) = &app.profile_name {
        let count = super::text::wrap_value(
            &format!("Name: {}", super::clean(name)),
            usize::from(shell.body.width),
        )
        .len();
        shell = super::view::measure(bounds, title(app), width, count, hint);
    }
    shell
}

pub(super) fn render(frame: &mut Frame, app: &App, shell: &modal::Layout<()>, theme: &Theme) {
    modal::render_measured(frame, shell, title(app), theme, Role::Header);
    let hint = if let Some(name) = &app.profile_name {
        let lines = super::text::wrap_value(
            &format!("Name: {}", super::clean(name)),
            usize::from(shell.body.width),
        );
        frame.render_widget(
            Paragraph::new(lines.join("\n")).style(theme.style(Role::Value)),
            shell.body,
        );
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
