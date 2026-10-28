//! Shared measured shell for the bounded pattern-selection form.
use super::modal::{self, Row, actions::Action};
use crate::{
    app::{App, PatternMode, PatternOperation},
    color::{Role, Theme},
};
use crossterm::event::KeyCode;
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

/// Exact rendered form target, shared by keyboard focus and mouse input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatternControl {
    Expression,
    Mode,
    Case,
    Apply(PatternOperation),
    Cancel,
}

/// One measured layout for drawing, input eligibility and hit testing.
#[derive(Clone, Debug)]
pub struct PatternLayout {
    pub shell: modal::Layout<PatternControl>,
    pub fields: Vec<(Rect, PatternControl)>,
    pub scroll_limit: usize,
    rows: Vec<Row<PatternControl>>,
    scroll: usize,
    summary: Vec<Row<PatternControl>>,
}

pub(super) fn layout(app: &App, area: Rect) -> Option<PatternLayout> {
    let state = app.pattern.as_ref()?;
    let preview = state.preview(&app.panes[state.pane]);
    let rows = rows(state);
    let summary = summary(preview.as_ref().map_err(String::as_str));
    let actions = actions(preview.is_ok());
    let (shell, footer_rows) = measure(area, &rows, &summary, &actions);
    let rows: Vec<_> = rows
        .into_iter()
        .flat_map(|row| row.wrap(shell.body.width))
        .collect();
    let scroll_limit = shell.scroll_limit(rows.len());
    let scroll = state.scroll.min(scroll_limit);
    let fields = fields(&rows, &shell, scroll);
    let shell = guard_actions(shell, footer_rows, &fields);
    Some(PatternLayout {
        shell,
        fields,
        rows,
        scroll,
        scroll_limit,
        summary,
    })
}

fn rows(state: &crate::app::PatternSelection) -> Vec<Row<PatternControl>> {
    vec![
        Row::field(
            "Expression",
            state.expression(),
            PatternControl::Expression,
            state.focus == 0,
        ),
        Row::field(
            "Mode",
            if state.mode == PatternMode::Wildcard {
                "Wildcard"
            } else {
                "Regex"
            },
            PatternControl::Mode,
            state.focus == 1,
        ),
        Row::field(
            "Case sensitive",
            if state.case_sensitive { "Yes" } else { "No" },
            PatternControl::Case,
            state.focus == 2,
        ),
        Row::plain("Wildcard: whole filename (* / ? bytes)."),
        Row::plain("Regex: substring; ^ / $ anchor filename."),
    ]
}

fn actions(ready: bool) -> Vec<Action<PatternControl>> {
    use PatternControl::*;
    [
        ("Mark matches", Apply(PatternOperation::Mark)),
        ("Unmark matches", Apply(PatternOperation::Unmark)),
        (
            "Replace visible marks with matches",
            Apply(PatternOperation::Replace),
        ),
        ("Cancel", Cancel),
    ]
    .into_iter()
    .map(|(label, intent)| {
        let mut action = Action::custom(label, intent, KeyCode::Null);
        action.enter = true;
        action.enabled = ready || intent == Cancel;
        action
    })
    .collect()
}

impl PatternLayout {
    /// Visible enabled targets in focus order.
    pub fn targets(&self) -> Vec<PatternControl> {
        self.fields
            .iter()
            .map(|(_, target)| *target)
            .chain(
                self.shell
                    .actions
                    .iter()
                    .filter(|r| r.action.enabled)
                    .map(|r| r.action.intent),
            )
            .collect()
    }

    /// Only rectangles actually rendered in this layout are interactive.
    pub fn hit(&self, column: u16, row: u16) -> Option<PatternControl> {
        self.shell.hit(column, row).copied().or_else(|| {
            self.fields
                .iter()
                .find(|(area, _)| area.contains((column, row).into()))
                .map(|(_, target)| *target)
        })
    }
}

pub(super) fn render(frame: &mut Frame, app: &App, theme: &Theme, layout: Option<&PatternLayout>) {
    let Some(layout) = layout else { return };
    let state = app.pattern.as_ref().unwrap();
    modal::render_shell(frame, layout.shell.outer, "Pattern selection", theme);
    for (i, row) in layout
        .rows
        .iter()
        .skip(layout.scroll)
        .take(usize::from(layout.shell.body.height))
        .enumerate()
    {
        frame.render_widget(
            Paragraph::new(row.line(layout.shell.body.width, theme)),
            Rect::new(
                layout.shell.body.x,
                layout.shell.body.y + i as u16,
                layout.shell.body.width,
                1,
            ),
        );
    }
    let lines: Vec<_> = layout
        .summary
        .iter()
        .cloned()
        .flat_map(|row| row.wrap(layout.shell.footer.width))
        .map(|row| row.line(layout.shell.footer.width, theme))
        .collect();
    frame.render_widget(Paragraph::new(lines), layout.shell.footer);
    modal::actions::render(frame, &layout.shell, theme, state.focus.checked_sub(3));
}

fn summary(preview: Result<&crate::app::PatternPreview, &str>) -> Vec<Row<PatternControl>> {
    match preview {
        Ok(p) => vec![
            Row::plain(format!("Matches: {} / {} selectable", p.matches, p.visible)),
            Row::styled(
                format!(
                    "Replace: {} visible marks -> {} matches",
                    p.marked, p.matches
                ),
                Role::Warning,
            ),
            Row::plain("Excluded marks stay. Tab selects; Enter activates."),
        ],
        Err(error) => vec![
            Row::styled(
                error.lines().last().unwrap_or("Invalid pattern"),
                Role::Error,
            ),
            Row::plain("Esc cancels without changing marks."),
        ],
    }
}

fn fields(
    rows: &[Row<PatternControl>],
    shell: &modal::Layout<PatternControl>,
    scroll: usize,
) -> Vec<(Rect, PatternControl)> {
    rows.iter()
        .skip(scroll)
        .take(usize::from(shell.body.height))
        .enumerate()
        .filter_map(|(i, row)| {
            row.field.map(|target| {
                (
                    Rect::new(shell.body.x, shell.body.y + i as u16, shell.body.width, 1),
                    target,
                )
            })
        })
        .collect()
}

fn guard_actions(
    mut shell: modal::Layout<PatternControl>,
    footer_rows: u16,
    fields: &[(Rect, PatternControl)],
) -> modal::Layout<PatternControl> {
    let ready = shell.footer.height == footer_rows
        && fields
            .iter()
            .any(|(_, target)| *target == PatternControl::Expression);
    for region in &mut shell.actions {
        if matches!(region.action.intent, PatternControl::Apply(_)) {
            region.action.enabled &= ready;
        }
    }
    shell
}

fn measure(
    area: Rect,
    rows: &[Row<PatternControl>],
    summary: &[Row<PatternControl>],
    actions: &[Action<PatternControl>],
) -> (modal::Layout<PatternControl>, u16) {
    let content_width = rows
        .iter()
        .chain(summary)
        .map(Row::width)
        .max()
        .unwrap_or(0)
        .min(68);
    // Bound the action group by content width; shared packing wraps whole buttons.
    let width = (content_width as u16 + 6).min(area.width);
    let bounds = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y,
        width,
        area.height,
    );
    let footer_rows = summary
        .iter()
        .cloned()
        .flat_map(|row| row.wrap(width.saturating_sub(6)))
        .count()
        .min(usize::from(u16::MAX)) as u16;
    let shell = modal::Spec {
        title: "Pattern selection",
        content_width,
        pinned_rows: 0,
        footer_rows,
        actions,
    }
    .measure(bounds, |width| {
        rows.iter().cloned().flat_map(|row| row.wrap(width)).count()
    });
    (shell, footer_rows)
}
