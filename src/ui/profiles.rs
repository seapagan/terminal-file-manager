use crate::{app::App, color::Theme};
use ratatui::{Frame, layout::Rect};

pub(super) fn render(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let mut rows: Vec<_> = app
        .profile_names
        .iter()
        .map(|name| (name.clone(), "Enter: active; Space: both…".into()))
        .collect();
    rows.push((
        "Apply selected to both".into(),
        app.profile_names
            .get(app.profile_choice)
            .cloned()
            .unwrap_or_default(),
    ));
    rows.push(("Save active as…".into(), "Enter: name then save".into()));
    let title = app.profile_name.as_ref().map_or_else(
        || "View profiles — Enter applies; Esc closes".into(),
        |name| {
            format!(
                "Save profile: {} — Enter saves; Esc cancels",
                super::clean(name)
            )
        },
    );
    super::view::menu(frame, area, rows, app.profile_menu.unwrap(), theme, &title);
}
