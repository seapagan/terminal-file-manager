//! Shared measured shell for the bounded pattern-selection form.
use super::modal::{self, Row, actions::Action};
use crate::{
    app::{App, PatternMode, PatternOperation},
    color::{Role, Theme},
};
use crossterm::event::KeyCode;
use ratatui::{Frame, layout::Rect, style::Modifier, widgets::Paragraph};

pub use crate::app::PatternControl;

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
    let preview = state.cached_preview(&app.panes[state.pane]);
    let rows = rows(state, area.width.saturating_sub(6).min(70));
    let mut summary = summary_rows(preview.as_ref().map_err(String::as_str));
    if let Some(feedback) = &state.feedback {
        summary[2] = Row::styled(feedback, Role::Warning).single_line();
    }
    let actions = actions(preview.is_ok());
    let bound = app.panes[state.pane].entries.len();
    let summary_budget = summary_rows(Ok(&crate::app::PatternPreview {
        matches: bound,
        visible: bound,
        marked: bound,
    }));
    let shell = measure(area, &rows, &summary_budget, &actions);
    let rows: Vec<_> = rows
        .into_iter()
        .flat_map(|row| row.wrap(shell.body.width))
        .collect();
    let scroll_limit = shell.scroll_limit(rows.len());
    let scroll = state.scroll.min(scroll_limit);
    let mut fields = fields(&rows, &shell, scroll);
    let expression_row = rows
        .iter()
        .position(|row| row.field == Some(PatternControl::Expression));
    if let Some(expression_row) = expression_row {
        for (mut area, target) in controls(shell.body.width) {
            let row = expression_row + 2 + usize::from(area.y);
            if row >= scroll && row - scroll < usize::from(shell.body.height) {
                area.x += shell.body.x;
                area.y = shell.body.y + (row - scroll) as u16;
                fields.push((area, target));
            }
        }
    }
    let shell = guard_actions(shell, &summary, &fields);
    Some(PatternLayout {
        shell,
        fields,
        rows,
        scroll,
        scroll_limit,
        summary,
    })
}

fn rows(state: &crate::app::PatternSelection, width: u16) -> Vec<Row<PatternControl>> {
    let mut rows = vec![
        Row::field(
            "Pattern",
            state.expression(),
            PatternControl::Expression,
            state.focus == PatternControl::Expression,
        ),
        Row::plain(""),
    ];
    let controls = controls(width);
    let count = controls.last().map_or(0, |(area, _)| area.y + 1);
    rows.extend((0..count).map(|_| Row::plain("")));
    let explanations = [
        "Wildcard: * matches a sequence; ? matches one byte.",
        "Regex searches within filenames; ^ / $ anchor the match.",
    ];
    let reserved = explanations
        .iter()
        .map(|text| Row::<PatternControl>::plain(*text).wrap(width).len())
        .max()
        .unwrap_or(0);
    let active = usize::from(state.mode == PatternMode::Regex);
    let explanation = Row::plain(explanations[active]).wrap(width);
    let padding = reserved.saturating_sub(explanation.len());
    rows.extend(explanation);
    rows.extend((0..padding).map(|_| Row::plain("")));
    let hints = [
        "While typing: Alt+letter activates controls",
        "Outside input: M/U/R apply; click Pattern to type",
    ];
    let hint = usize::from(state.focus != PatternControl::Expression);
    let reserved = hints
        .iter()
        .map(|text| Row::<PatternControl>::plain(*text).wrap(width).len())
        .max()
        .unwrap_or(0);
    let hint = Row::plain(hints[hint]).wrap(width);
    let padding = reserved.saturating_sub(hint.len());
    rows.extend(hint);
    rows.extend((0..padding).map(|_| Row::plain("")));
    rows
}

fn controls(width: u16) -> Vec<(Rect, PatternControl)> {
    let mut controls = Vec::new();
    let (mut x, mut y) = (0, 0);
    for (label, target) in [
        ("W Wildcard", PatternControl::Mode(PatternMode::Wildcard)),
        ("X Regex", PatternControl::Mode(PatternMode::Regex)),
        ("S Case sensitive", PatternControl::Case),
    ] {
        let cells = modal::width(label) + 4;
        if cells > width {
            continue;
        }
        if x + cells > width {
            x = 0;
            y += 1;
        }
        controls.push((Rect::new(x, y, cells, 1), target));
        x += cells + 2;
    }
    for row in 0..=y {
        let used = controls
            .iter()
            .filter(|(r, _)| r.y == row)
            .map(|(r, _)| r.right())
            .max()
            .unwrap_or(0);
        for (area, _) in controls.iter_mut().filter(|(r, _)| r.y == row) {
            area.x += width.saturating_sub(used) / 2;
        }
    }
    controls
}

fn actions(ready: bool) -> Vec<Action<PatternControl>> {
    use PatternControl::*;
    [
        ("M Mark", Apply(PatternOperation::Mark)),
        ("U Unmark", Apply(PatternOperation::Unmark)),
        ("R Replace", Apply(PatternOperation::Replace)),
        ("Esc Cancel", Cancel),
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
            Paragraph::new(row.line(layout.shell.body.width, theme))
                .style(theme.style(Role::Normal)),
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
    render_controls(frame, state, theme, layout);
    modal::actions::render(
        frame,
        &layout.shell,
        theme,
        layout
            .shell
            .actions
            .iter()
            .position(|r| r.action.intent == state.focus && r.action.enabled),
    );
}

fn render_controls(
    frame: &mut Frame,
    state: &crate::app::PatternSelection,
    theme: &Theme,
    layout: &PatternLayout,
) {
    for (area, target) in &layout.fields {
        if *target == PatternControl::Expression {
            continue;
        }
        let (label, selected, focused) = match target {
            PatternControl::Mode(PatternMode::Wildcard) => (
                "W Wildcard",
                state.mode == PatternMode::Wildcard,
                state.focus == *target,
            ),
            PatternControl::Mode(PatternMode::Regex) => (
                "X Regex",
                state.mode == PatternMode::Regex,
                state.focus == *target,
            ),
            PatternControl::Case => (
                "S Case sensitive",
                state.case_sensitive,
                state.focus == *target,
            ),
            _ => continue,
        };
        let mut style = theme.style(if selected { Role::Key } else { Role::Normal });
        if selected {
            style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }
        if focused {
            style = style.add_modifier(Modifier::REVERSED);
        }
        let marker = control_marker(*target, selected);
        frame.render_widget(
            Paragraph::new(format!("{marker} {label}")).style(style),
            *area,
        );
    }
}

fn control_marker(target: PatternControl, selected: bool) -> &'static str {
    match (target, selected) {
        (PatternControl::Case, true) => "[x]",
        (PatternControl::Case, false) => "[ ]",
        (_, true) => "(●)",
        (_, false) => "( )",
    }
}

fn summary_rows(preview: Result<&crate::app::PatternPreview, &str>) -> Vec<Row<PatternControl>> {
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
            Row::plain("Tab selects; Enter activates. Esc cancels."),
        ],
        Err(error) => vec![
            Row::styled(
                error.lines().last().unwrap_or("Invalid pattern"),
                Role::Error,
            )
            .single_line(),
            Row::plain(""),
            Row::plain("Tab selects; Enter activates. Esc cancels."),
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
    summary: &[Row<PatternControl>],
    fields: &[(Rect, PatternControl)],
) -> modal::Layout<PatternControl> {
    let ready = shell.body.width >= 12
        && [
            PatternControl::Expression,
            PatternControl::Mode(PatternMode::Wildcard),
            PatternControl::Mode(PatternMode::Regex),
            PatternControl::Case,
        ]
        .iter()
        .all(|target| fields.iter().any(|(_, field)| field == target));
    for region in &mut shell.actions {
        if let PatternControl::Apply(operation) = region.action.intent {
            // Replace additionally needs its complete consequence, not the usage hint.
            let required = if operation == PatternOperation::Replace {
                2
            } else {
                1
            };
            let context_rows = summary
                .iter()
                .take(required)
                .cloned()
                .flat_map(|row| row.wrap(shell.footer.width))
                .count();
            region.action.enabled &= ready && context_rows <= usize::from(shell.footer.height);
        }
    }
    shell
}

fn measure(
    area: Rect,
    rows: &[Row<PatternControl>],
    summary: &[Row<PatternControl>],
    actions: &[Action<PatternControl>],
) -> modal::Layout<PatternControl> {
    let content_width = 70;
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
    modal::Spec {
        title: "Pattern selection",
        content_width,
        pinned_rows: 0,
        footer_rows,
        actions,
    }
    .measure(bounds, |width| {
        rows.iter().cloned().flat_map(|row| row.wrap(width)).count()
    })
}
