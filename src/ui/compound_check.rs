//! Consecutive startup dialogs share measured content, actions and hit testing.
use super::modal::{self, actions::Action};
use crate::{
    app::App,
    color::{Role, Theme},
    settings::compound_check::{Review, Stage, difference},
};
use crossterm::event::KeyCode;
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

const HINT: &str = "Tab/←→: choose  Enter: select  Esc: keep";
const PREFERRED_WIDTH: u16 = 56;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckIntent {
    Keep,
    Defaults,
    Remove,
    Dismiss,
}

#[derive(Clone, Debug)]
pub struct CheckLayout {
    pub shell: modal::Layout<CheckIntent>,
    pub stage: Stage,
    pub scroll: usize,
    pub limit: usize,
}

fn title(stage: Stage) -> &'static str {
    match stage {
        Stage::Replacement => "Compound extension compatibility",
        Stage::Additions => "Review custom extensions",
    }
}

pub(crate) fn actions(review: &Review) -> Vec<Action<CheckIntent>> {
    let choices = match review.stage {
        Stage::Replacement => [
            ("Keep my configuration", CheckIntent::Keep, 'k'),
            ("Use current defaults", CheckIntent::Defaults, 'd'),
        ],
        Stage::Additions => [
            ("Keep additions", CheckIntent::Keep, 'k'),
            ("Remove duplicates", CheckIntent::Remove, 'r'),
        ],
    };
    choices
        .into_iter()
        .map(|(label, intent, key)| {
            let mut action = Action::custom(label, intent, KeyCode::Char(key));
            action.enter = true;
            action
        })
        .collect()
}

fn prose(rows: &mut Vec<modal::Row<()>>, text: &str, role: Role, width: u16) {
    rows.extend(
        super::text::wrap_words(text, usize::from(width))
            .into_iter()
            .map(|text| modal::Row::styled(text, role)),
    );
}

fn group(
    rows: &mut Vec<modal::Row<()>>,
    label: &str,
    values: &[String],
    marker: &str,
    role: Role,
    width: u16,
) {
    prose(rows, label, Role::SectionHeader, width);
    if values.is_empty() {
        prose(rows, "  No compound extensions", Role::Muted, width);
    } else {
        rows.extend(value_rows(values, marker, role, width));
    }
}

fn value_rows(values: &[String], marker: &str, role: Role, width: u16) -> Vec<modal::Row<()>> {
    let indent = " ".repeat(usize::from(width.min(2)));
    let available = width.saturating_sub(2);
    let mut rows = Vec::new();
    let mut row = String::new();
    for value in values {
        let token = format!("{marker}{}", super::clean(value));
        let gap = if row.is_empty() { "" } else { "   " };
        if modal::width(&row)
            .saturating_add(modal::width(gap))
            .saturating_add(modal::width(&token))
            > available
        {
            if !row.is_empty() {
                rows.push(modal::Row::styled(format!("{indent}{row}"), role));
                row.clear();
            }
            if modal::width(&token) > available {
                rows.extend(
                    super::text::wrap_value(&token, usize::from(available))
                        .into_iter()
                        .map(|part| modal::Row::styled(format!("{indent}{part}"), role)),
                );
                continue;
            }
        }
        if !row.is_empty() {
            row.push_str("   ");
        }
        row.push_str(&token);
    }
    if !row.is_empty() {
        rows.push(modal::Row::styled(format!("{indent}{row}"), role));
    }
    rows
}

fn replacement_rows(review: &Review, width: u16) -> Vec<modal::Row<()>> {
    let configured = review.configured.as_ref().unwrap();
    let mut rows = Vec::new();
    group(
        &mut rows,
        &format!("Your configured extensions ({})", configured.len()),
        configured,
        "",
        Role::Value,
        width,
    );
    rows.push(modal::Row::plain(""));
    group(
        &mut rows,
        &format!("Current TFM defaults ({})", review.defaults.len()),
        &review.defaults,
        "",
        Role::Value,
        width,
    );
    for (label, values, marker, role) in [
        (
            "Additional extensions in TFM's defaults",
            difference(&review.defaults, configured),
            "+ ",
            Role::Marked,
        ),
        (
            "Extensions removed by using defaults",
            difference(configured, &review.defaults),
            "- ",
            Role::Warning,
        ),
    ] {
        if !values.is_empty() {
            rows.push(modal::Row::plain(""));
            group(&mut rows, label, &values, marker, role, width);
        }
    }
    rows.push(modal::Row::plain(""));
    prose(
        &mut rows,
        "Which list would you like to use?",
        Role::Label,
        width,
    );
    rows
}

fn addition_rows(review: &Review, width: u16) -> Vec<modal::Row<()>> {
    let mut rows = Vec::new();
    prose(
        &mut rows,
        "These additions already occur in your chosen base list.",
        Role::Value,
        width,
    );
    rows.push(modal::Row::plain(""));
    group(
        &mut rows,
        &format!("Your added extensions ({})", review.additions.len()),
        &review.additions,
        "",
        Role::Value,
        width,
    );
    rows.push(modal::Row::plain(""));
    let removable = review.removable();
    group(
        &mut rows,
        "Already included",
        &removable,
        "✓ ",
        Role::Marked,
        width,
    );
    let custom = difference(&review.additions, &removable);
    if !custom.is_empty() {
        group(
            &mut rows,
            "Custom additions to keep",
            &custom,
            "",
            Role::Value,
            width,
        );
    }
    rows.push(modal::Row::plain(""));
    prose(
        &mut rows,
        "Removal keeps the same recognised compound extensions.",
        Role::Value,
        width,
    );
    rows
}

fn rows(review: &Review, width: u16) -> Vec<modal::Row<()>> {
    match review.stage {
        Stage::Replacement => replacement_rows(review, width),
        Stage::Additions => addition_rows(review, width),
    }
}

pub(super) fn layout(app: &App, area: Rect) -> Option<CheckLayout> {
    let review = app.compound_extension_review.as_ref()?;
    let actions = actions(review);
    let content_width = rows(review, PREFERRED_WIDTH)
        .iter()
        .map(modal::Row::width)
        .max()
        .unwrap_or(0);
    let spec = modal::Spec {
        title: title(review.stage),
        help: false,
        content_width,
        pinned_rows: 0,
        footer_rows: 0,
        actions: &actions,
    };
    let measured = spec.measure(area, |width| rows(review, width).len());
    let footer_rows = super::text::wrap_words(HINT, usize::from(measured.body.width)).len() as u16;
    let mut shell = modal::Spec {
        footer_rows,
        ..spec
    }
    .measure(area, |width| rows(review, width).len());
    if shell.body.height == 0 || shell.body.width == 0 {
        for region in &mut shell.actions {
            region.action.enabled &= region.action.intent == CheckIntent::Keep;
        }
    }
    let limit = shell.scroll_limit(rows(review, shell.body.width).len());
    Some(CheckLayout {
        stage: review.stage,
        scroll: review.scroll.min(limit),
        shell,
        limit,
    })
}

pub(super) fn render(frame: &mut Frame, app: &App, geometry: &super::Geometry, theme: &Theme) {
    let Some(layout) = &geometry.compound_extension_review else {
        return;
    };
    let review = app.compound_extension_review.as_ref().unwrap();
    modal::render_measured(
        frame,
        &layout.shell,
        title(review.stage),
        theme,
        Role::Header,
    );
    let lines: Vec<_> = rows(review, layout.shell.body.width)
        .into_iter()
        .skip(layout.scroll)
        .take(usize::from(layout.shell.body.height))
        .map(|row| row.line(layout.shell.body.width, theme))
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
                "{} / {} · ↑↓ / PgUp / PgDn",
                layout.scroll + 1,
                layout.limit + 1
            ))
            .style(theme.style(Role::Muted)),
            layout.shell.cue,
        );
    }
    modal::actions::render(frame, &layout.shell, theme, Some(review.selected));
}
