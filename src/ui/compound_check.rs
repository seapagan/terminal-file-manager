//! Startup compatibility uses the shared measured, scrollable modal.
use super::modal::{self, actions::Action};
use crate::{
    app::App,
    color::{Role, Theme},
    settings::compound_check::{Review, difference},
};
use crossterm::event::KeyCode;
use ratatui::{Frame, layout::Rect, text::Line, widgets::Paragraph};

const TITLE: &str = "Compound extension compatibility";
const HINT: &str = "Tab/Left/Right: actions; Space/Enter: choose; Up/Down/PgUp/PgDn: scroll";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckIntent {
    Keep,
    Defaults,
    Remove,
    Continue,
    Dismiss,
}

#[derive(Clone, Debug)]
pub struct CheckLayout {
    pub shell: modal::Layout<CheckIntent>,
    pub scroll: usize,
    pub limit: usize,
}

pub(crate) fn actions(review: &Review) -> Vec<Action<CheckIntent>> {
    let mut actions = Vec::new();
    if review.replacement_differs() {
        actions.push(choice(
            "Keep my configuration",
            CheckIntent::Keep,
            'k',
            !review.use_defaults,
        ));
        actions.push(choice(
            "Use current defaults",
            CheckIntent::Defaults,
            'd',
            review.use_defaults,
        ));
    }
    if !review.default_additions().is_empty() {
        let mut remove = choice(
            "Remove redundant entries",
            CheckIntent::Remove,
            'r',
            review.remove_redundant,
        );
        remove.enabled = !review.removable().is_empty();
        actions.push(remove);
    }
    let mut done = Action::custom("Continue", CheckIntent::Continue, KeyCode::Enter);
    done.enter = true;
    actions.push(done);
    let mut dismiss = Action::custom("Esc Keep both lists", CheckIntent::Dismiss, KeyCode::Esc);
    dismiss.enter = true;
    actions.push(dismiss);
    actions
}

fn choice(label: &str, intent: CheckIntent, key: char, chosen: bool) -> Action<CheckIntent> {
    let state = if chosen { "On" } else { "Off" };
    let mut action = Action::custom(
        format!("{key} {label}: {state}"),
        intent,
        KeyCode::Char(key),
    );
    action.state = Some((state, Role::Value));
    action.enter = true;
    action
}

fn list(values: &[String]) -> String {
    format!("{values:?}")
}

fn rows(review: &Review) -> Vec<modal::Row<()>> {
    let mut rows = vec![modal::Row::plain(format!(
        "Review compound extensions for TFM {}. Continue saves your choices; Esc keeps both lists.",
        review.version
    ))];
    if review.replacement_differs() {
        let configured = review.configured.as_ref().unwrap();
        rows.extend([
            modal::Row::styled("Your configured extensions:", Role::Label),
            modal::Row::plain(list(configured)),
            modal::Row::styled("Current TFM defaults:", Role::Label),
            modal::Row::plain(list(&review.defaults)),
            modal::Row::styled("Differences:", Role::Label),
            modal::Row::key_value(
                "Only in your configuration",
                list(&difference(configured, &review.defaults)),
            ),
            modal::Row::key_value(
                "Only in current defaults",
                list(&difference(&review.defaults, configured)),
            ),
        ]);
    }
    if !review.default_additions().is_empty() {
        let removable = review.removable();
        rows.extend([
            modal::Row::styled("Additive entries now in TFM defaults:", Role::Label),
            modal::Row::plain(list(&review.default_additions())),
            modal::Row::key_value("Safe to remove with your current choice", list(&removable)),
            modal::Row::key_value(
                "Retain to preserve your chosen replacement",
                list(&difference(&review.default_additions(), &removable)),
            ),
            modal::Row::plain(
                "Removal keeps unrelated custom entries. Leave removal Off to keep all additions.",
            ),
        ]);
    }
    rows
}

fn lines(review: &Review, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    rows(review)
        .into_iter()
        .flat_map(|row| row.wrap(width))
        .map(|row| row.line(width, theme))
        .collect()
}

pub(super) fn layout(app: &App, area: Rect) -> Option<CheckLayout> {
    let review = app.compound_extension_review.as_ref()?;
    let actions = actions(review);
    let mut shell = modal::Spec {
        title: TITLE,
        help: false,
        content_width: 88,
        pinned_rows: 0,
        footer_rows: 2,
        actions: &actions,
    }
    .measure(area, |width| {
        rows(review).into_iter().flat_map(|r| r.wrap(width)).count()
    });
    if shell.body.height == 0 || shell.body.width == 0 {
        for region in &mut shell.actions {
            region.action.enabled &= region.action.intent == CheckIntent::Dismiss;
        }
    }
    let count = rows(review)
        .into_iter()
        .flat_map(|r| r.wrap(shell.body.width))
        .count();
    let limit = shell.scroll_limit(count);
    Some(CheckLayout {
        shell,
        scroll: review.scroll.min(limit),
        limit,
    })
}

pub(super) fn render(frame: &mut Frame, app: &App, geometry: &super::Geometry, theme: &Theme) {
    let Some(layout) = &geometry.compound_extension_review else {
        return;
    };
    let review = app.compound_extension_review.as_ref().unwrap();
    modal::render_measured(frame, &layout.shell, TITLE, theme, Role::Header);
    let lines: Vec<_> = lines(review, layout.shell.body.width, theme)
        .into_iter()
        .skip(layout.scroll)
        .take(usize::from(layout.shell.body.height))
        .collect();
    frame.render_widget(Paragraph::new(lines), layout.shell.body);
    frame.render_widget(
        Paragraph::new(
            super::text::wrap_words(HINT, usize::from(layout.shell.footer.width)).join("\n"),
        )
        .style(theme.style(Role::Key)),
        layout.shell.footer,
    );
    if layout.limit > 0 {
        frame.render_widget(
            Paragraph::new(format!(
                "{} / {} (scroll for all values)",
                layout.scroll + 1,
                layout.limit + 1
            ))
            .style(theme.style(Role::Muted)),
            layout.shell.cue,
        );
    }
    modal::actions::render(frame, &layout.shell, theme, Some(review.selected));
}
