//! Read-only contextual overlay uses the same measured shell as its parent.
use crate::{
    app::App,
    color::{Role, Theme},
    ui::{modal, text::wrap_value},
};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

#[derive(Clone, Debug)]
pub struct ContextLayout {
    pub shell: modal::Layout<()>,
    pub scroll_limit: u16,
    rows: Vec<String>,
}

pub(in crate::ui) fn layout(app: &App, bounds: Rect) -> Option<ContextLayout> {
    let state = app.context_help.as_ref()?;
    let paragraphs = state.topic.text(app);
    let actions = [modal::actions::Action::custom(
        "Esc Back",
        (),
        crossterm::event::KeyCode::Esc,
    )];
    let mut rows = Vec::new();
    let shell = modal::Spec {
        title: state.topic.title(),
        help: false,
        content_width: 64,
        pinned_rows: 0,
        footer_rows: 1,
        actions: &actions,
    }
    .measure(bounds, |width| {
        for paragraph in &paragraphs {
            if !rows.is_empty() {
                rows.push(String::new());
            }
            rows.extend(wrap_value(paragraph, usize::from(width)));
        }
        rows.len()
    });
    let scroll_limit = shell.scroll_limit(rows.len()).min(usize::from(u16::MAX)) as u16;
    Some(ContextLayout {
        shell,
        scroll_limit,
        rows,
    })
}

pub(in crate::ui) fn render(
    frame: &mut Frame,
    app: &App,
    theme: &Theme,
    layout: Option<&ContextLayout>,
) {
    let Some(layout) = layout else {
        return;
    };
    let state = app.context_help.as_ref().expect("context Help");
    modal::render_measured(
        frame,
        &layout.shell,
        state.topic.title(),
        theme,
        Role::Header,
    );
    frame.render_widget(
        Paragraph::new(layout.rows.join("\n"))
            .style(theme.style(Role::Value))
            .scroll((state.scroll.min(layout.scroll_limit), 0)),
        layout.shell.body,
    );
    let hint = format!(
        "↑/↓ wheel · PageUp/PageDown · {}/{}",
        state.scroll.min(layout.scroll_limit) + 1,
        layout.scroll_limit + 1
    );
    frame.render_widget(
        Paragraph::new(modal::truncate(&hint, layout.shell.footer.width))
            .style(theme.style(Role::Key)),
        layout.shell.footer,
    );
    modal::actions::render(frame, &layout.shell, theme, Some(0));
}
