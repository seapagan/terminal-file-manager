use super::{OperationTarget, content};
use crate::app::App;
use ratatui::layout::Rect;

pub(super) fn layout(app: &App, terminal: Rect) -> super::Layout {
    use crate::ui::modal;
    let outer = viewport(app, terminal);
    let height = outer.height;
    let narrow = terminal.height < 18 || terminal.width < 44 || app.operations.history.is_empty();
    let actual = super::actions::buttons(app, true);
    let primary = actions(&actual[..2]);
    let context = actions(&actual[2..]);
    let spec = modal::Spec {
        title: content::title(app),
        content_width: 0,
        pinned_rows: 3.min(height / 5),
        footer_rows: 2.min(height / 5),
        actions: &primary,
    };
    let mut shell =
        modal::Layout::viewport_with_action_rows(&spec, outer, if narrow { 1 } else { 2 });
    let context_fits = !narrow && context_width(&context) <= shell.body.width;
    if context_fits {
        let contextual = modal::Layout::viewport(
            &modal::Spec {
                actions: &context,
                ..spec
            },
            outer,
        );
        shell
            .actions
            .extend(contextual.actions.into_iter().map(|mut region| {
                region.index += 2;
                region
            }));
    }
    let rows = if app.operations.jobs_evidence {
        super::jobs::details(app)
    } else {
        super::jobs_summary::summary(app, shell.body.width)
    };
    let rows = content::wrapped(rows, shell.body.width);
    let hidden = if context_fits {
        &actual[0..0]
    } else {
        &actual[2..]
    };
    let footer = super::jobs_summary::footer(app, shell.footer, hidden);
    super::Layout::measured(app, shell, Vec::new(), footer, rows)
}

fn viewport(app: &App, terminal: Rect) -> Rect {
    let width = (terminal.width.saturating_mul(3) / 4)
        .clamp(60, 116)
        .min(terminal.width.saturating_sub(2));
    let height = (terminal.height.saturating_mul(3) / 4 + 1)
        .min(if app.operations.history.is_empty() {
            12
        } else {
            22
        })
        .min(terminal.height.saturating_sub(2));
    let available = Rect {
        height: terminal.height.saturating_sub(2),
        ..terminal
    };
    crate::ui::centered(available, width, height)
}

fn actions(
    buttons: &[(&'static str, crossterm::event::KeyCode)],
) -> Vec<crate::ui::modal::actions::Action<OperationTarget>> {
    use crate::ui::modal::{self, actions::Action};
    buttons
        .iter()
        .map(|&(label, key)| {
            let mut action = Action::custom(label, OperationTarget::Key(key), key);
            if key == crossterm::event::KeyCode::Char('e') {
                action.min_width = modal::width("E Evidence");
            }
            action
        })
        .collect()
}

fn context_width(actions: &[crate::ui::modal::actions::Action<OperationTarget>]) -> u16 {
    actions
        .iter()
        .map(|a| crate::ui::modal::width(a.label).saturating_add(3))
        .fold(0u16, u16::saturating_add)
        .saturating_sub(1)
}
