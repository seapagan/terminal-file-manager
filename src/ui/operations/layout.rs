use super::{OperationControl, OperationTarget, actions, content};
use crate::{
    app::App,
    ui::modal::{self, actions::Action},
};
use ratatui::layout::Rect;

#[derive(Clone, Debug)]
pub(in crate::ui) struct Layout {
    pub outer: Rect,
    pub body: Rect,
    pub list: Rect,
    pub buttons: Vec<OperationControl>,
    pub shell: modal::Layout<OperationTarget>,
    pub(super) header: Vec<content::Row>,
    pub(super) footer: Vec<content::Row>,
    pub(super) rows: Vec<content::Row>,
}

impl Layout {
    pub fn new(app: &App, terminal: Rect) -> Self {
        if app.operations.show_history && app.operations.dialog.is_none() {
            return super::jobs_layout::layout(app, terminal);
        }
        Self::dialog(app, terminal)
    }

    fn dialog(app: &App, terminal: Rect) -> Self {
        let rows = content::rows(app);
        let header = content::header(app);
        let footer = content::footer(app);
        let buttons = buttons(app, terminal.width);
        let spec = modal::Spec {
            title: content::title(app),
            content_width: desired_width(&rows, terminal.width)
                .max(desired_width(&header, terminal.width))
                .max(footer_width(app, &footer, terminal.width))
                .max(summary_width(app)),
            pinned_rows: list_rows(app, terminal.height),
            footer_rows: 0,
            actions: &buttons,
        };
        let width = spec.measure(terminal, |_| 0).body.width;
        let summary_visible = header
            .first()
            .is_none_or(|row| row.width() <= usize::from(width));
        let mut header = content::wrapped(header, width);
        let divider = add_divider(app, &rows, &mut header, width);
        let (mut footer, padding) = wrapped_footer(app, footer, width);
        let mut spec = modal::Spec {
            pinned_rows: spec
                .pinned_rows
                .saturating_add(header.len().min(usize::from(u16::MAX)) as u16),
            footer_rows: footer.len().min(usize::from(u16::MAX)) as u16,
            ..spec
        };
        let wrapped = content::wrapped(rows, width);
        let mut shell = spec.measure(terminal, |_| wrapped.len());
        collapse_padding(
            &mut spec,
            &mut shell,
            &mut footer,
            terminal,
            wrapped.len(),
            padding,
        );
        if divider && shell.body.height == 0 {
            header.pop();
            spec.pinned_rows = spec.pinned_rows.saturating_sub(1);
            shell = spec.measure(terminal, |_| wrapped.len());
        }
        let complete =
            shell.footer.height == spec.footer_rows && shell.pinned.height == spec.pinned_rows;
        guard_approval(app, &mut shell, &wrapped, complete && summary_visible);
        Self::measured(app, shell, header, footer, wrapped)
    }

    pub(super) fn measured(
        app: &App,
        shell: modal::Layout<OperationTarget>,
        header: Vec<content::Row>,
        footer: Vec<content::Row>,
        rows: Vec<content::Row>,
    ) -> Self {
        Self {
            outer: shell.outer,
            body: shell.body,
            list: history_area(app, &shell),
            buttons: controls(&shell),
            shell,
            header,
            footer,
            rows,
        }
    }

    pub fn limit(&self, rows: usize) -> usize {
        self.shell.scroll_limit(rows)
    }
}

fn collapse_padding(
    spec: &mut modal::Spec<'_, OperationTarget>,
    shell: &mut modal::Layout<OperationTarget>,
    footer: &mut Vec<content::Row>,
    terminal: Rect,
    rows: usize,
    padding: usize,
) {
    if padding > 0 && shell.body.height == 0 {
        footer.drain(..padding);
        spec.footer_rows = footer.len() as u16;
        *shell = spec.measure(terminal, |_| rows);
    }
}

fn footer_width(app: &App, rows: &[content::Row], width: u16) -> usize {
    let measured = desired_width(rows, width);
    let summary = summary_width(app);
    if summary > 0 {
        measured.min(summary)
    } else {
        measured
    }
}

fn wrapped_footer(app: &App, rows: Vec<content::Row>, width: u16) -> (Vec<content::Row>, usize) {
    let mut rows = content::wrapped(rows, width);
    if summary_width(app) == 0 {
        return (rows, 0);
    }
    // Reserve the standard failed-preview message plus one detail line so controls do not jump.
    let warnings = usize::from(
        matches!(&app.operations.dialog, Some(crate::operations::Dialog::Operation(d)) if d.plan.operation == crate::operations::Operation::Delete),
    ) * usize::from(modal::width(content::PERMANENT_WARNING))
        .div_ceil(usize::from(width).max(1));
    let capacity = usize::from(modal::width(content::PREVIEW_ERROR))
        .div_ceil(usize::from(width).max(1))
        + 1
        + warnings;
    let padding = capacity.saturating_sub(rows.len());
    rows.splice(0..0, std::iter::repeat_n(content::Row::plain(""), padding));
    (rows, padding)
}

fn add_divider(
    app: &App,
    rows: &[content::Row],
    header: &mut Vec<content::Row>,
    width: u16,
) -> bool {
    let divider = !rows.is_empty()
        && matches!(
            app.operations.dialog,
            Some(crate::operations::Dialog::Operation(_))
        );
    if divider {
        header.push(content::Row::styled(
            "─".repeat(usize::from(width)),
            crate::color::Role::Muted,
        ));
    }
    divider
}

fn guard_approval(
    app: &App,
    shell: &mut modal::Layout<OperationTarget>,
    rows: &[content::Row],
    complete: bool,
) {
    let start = app.operations.scroll.min(shell.scroll_limit(rows.len()));
    let operands_visible = shell.body.height > 0
        && rows
            .iter()
            .skip(start)
            .take(usize::from(shell.body.height))
            .any(|row| {
                row.trailing
                    .as_ref()
                    .is_some_and(|size| shell.body.width >= modal::width(size).saturating_add(7))
            });
    for button in &mut shell.actions {
        if matches!(
            button.action.intent,
            OperationTarget::Key(crossterm::event::KeyCode::Char('D' | 'T'))
        ) {
            button.action.enabled &= complete && operands_visible;
        }
    }
}

fn history_area(app: &App, shell: &modal::Layout<OperationTarget>) -> Rect {
    if app.operations.show_history && app.operations.dialog.is_none() {
        shell.pinned
    } else {
        Rect::default()
    }
}

fn buttons(app: &App, width: u16) -> Vec<Action<OperationTarget>> {
    actions::buttons(app, width < 44)
        .into_iter()
        .map(|(label, key)| {
            let mut action = Action::custom(label, OperationTarget::Key(key), key);
            action.role = actions::role(app, key);
            action.enabled = actions::enabled(app, key);
            if key == crossterm::event::KeyCode::F(2) {
                action.min_width = modal::width(if width < 44 {
                    "F2 SHA-256 off"
                } else {
                    "F2 Disable SHA-256"
                });
            }
            action
        })
        .collect()
}

fn controls(shell: &modal::Layout<OperationTarget>) -> Vec<OperationControl> {
    shell
        .actions
        .iter()
        .map(|button| OperationControl {
            area: button.area,
            target: button.action.intent,
            label: button.action.label,
            enabled: button.action.enabled,
        })
        .collect()
}

fn desired_width(rows: &[content::Row], terminal_width: u16) -> usize {
    let mut widest = 0;
    for row in rows {
        widest = widest.max(row.width());
        if widest >= usize::from(terminal_width) {
            break;
        }
    }
    widest
}

fn list_rows(app: &App, height: u16) -> u16 {
    if app.operations.show_history && app.operations.dialog.is_none() {
        app.operations
            .history
            .len()
            .min(5)
            .min(usize::from(height / 4)) as u16
    } else {
        0
    }
}

fn summary_width(app: &App) -> usize {
    if matches!(&app.operations.dialog, Some(crate::operations::Dialog::Operation(dialog)) if matches!(dialog.plan.operation, crate::operations::Operation::Trash | crate::operations::Operation::Delete))
    {
        usize::from(modal::width(
            "| Counting recursively… confirmation disabled",
        ))
    } else {
        0
    }
}
