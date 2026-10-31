//! Context Help stores only its own topic and scroll, leaving parent state in place.
use crate::{
    app::App,
    input::{Action, Controller},
    operations::{Dialog, Operation},
    ui::Geometry,
};
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpTopic {
    Pattern,
    Operation(Operation),
    Conflict,
    Jobs,
    Metadata,
    Exit,
    Incident,
}

#[derive(Clone, Debug)]
pub struct ContextHelp {
    pub topic: HelpTopic,
    pub scroll: u16,
}

impl HelpTopic {
    pub fn current(app: &App) -> Option<Self> {
        if app.help || app.diagnostics || app.view_menu.is_some() || app.profile_menu.is_some() {
            return None;
        }
        if app.popup {
            return Some(Self::Metadata);
        }
        match &app.operations.dialog {
            Some(Dialog::Operation(dialog)) => return Some(Self::Operation(dialog.plan.operation)),
            Some(Dialog::Exit { .. }) => return Some(Self::Exit),
            Some(Dialog::IncidentExit { .. }) => return Some(Self::Incident),
            None => {}
        }
        if app.operations.show_history {
            Some(Self::Jobs)
        } else if !app.operations.conflicts.is_empty() {
            Some(Self::Conflict)
        } else if app.operations.waiting_exit.is_some() {
            Some(Self::Exit)
        } else {
            app.pattern.as_ref().map(|_| Self::Pattern)
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Pattern => "Pattern Help",
            Self::Conflict => "Conflict Help",
            Self::Jobs => "Jobs Help",
            Self::Metadata => "Metadata Help",
            Self::Exit => "Safe Exit Help",
            Self::Incident => "Incident Help",
            Self::Operation(operation) => match operation {
                Operation::Copy => "Copy Help",
                Operation::Move => "Move Help",
                Operation::Mkdir => "Create Directory Help",
                Operation::CreateFile => "Create File Help",
                Operation::Rename => "Rename Help",
                Operation::Trash => "Trash Help",
                Operation::Delete => "Permanent Delete Help",
                Operation::Permissions(_) | Operation::ReadOnly(_) => "Attributes Help",
            },
        }
    }

    pub fn text(self, app: &App) -> Vec<String> {
        let (action, lines) = match self {
            Self::Pattern => (Action::PatternSelection, PATTERN),
            Self::Conflict => (Action::Jobs, CONFLICT),
            Self::Jobs => (Action::Jobs, JOBS),
            Self::Metadata => return metadata_text(app),
            Self::Exit => (Action::Quit, EXIT),
            Self::Incident => (Action::Quit, INCIDENT),
            Self::Operation(operation) => return operation_text(operation, app),
        };
        with_binding(action, lines, app)
    }
}

fn with_binding(action: Action, lines: &[&str], app: &App) -> Vec<String> {
    let mut text: Vec<_> = lines.iter().map(|line| (*line).to_owned()).collect();
    if let Some(entry) = app.help_rows.iter().find(|entry| entry.action == action) {
        text.push(format!(
            "Browser shortcut: {} — {}",
            entry.keys, entry.description
        ));
    }
    text
}

fn operation_text(operation: Operation, app: &App) -> Vec<String> {
    let (action, lines): (_, &[&str]) = match operation {
        Operation::Copy => (Action::Copy, TRANSFER),
        Operation::Move => (Action::Move, TRANSFER),
        Operation::Mkdir => (Action::Mkdir, CREATE),
        Operation::CreateFile => (Action::CreateFile, CREATE),
        Operation::Rename => (Action::Rename, RENAME),
        Operation::Trash => (Action::Trash, TRASH),
        Operation::Delete => (Action::Delete, DELETE),
        Operation::Permissions(_) | Operation::ReadOnly(_) => (Action::Attributes, ATTRIBUTES),
    };
    with_binding(action, lines, app)
}

pub(in crate::input) fn handle(c: &mut Controller, event: &Event, geometry: &Geometry) -> bool {
    if crate::input::operations::interrupt(event) {
        c.app.context_help = None;
        return false;
    }
    if c.app.context_help.is_some() {
        overlay(c, event, geometry);
        return true;
    }
    let Some(topic) = HelpTopic::current(&c.app) else {
        return false;
    };
    let open = match event {
        Event::Key(key) => {
            key.kind == KeyEventKind::Press && key.code == KeyCode::F(1) && key.modifiers.is_empty()
        }
        Event::Mouse(mouse) => {
            mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && geometry
                    .context_control
                    .is_some_and(|area| area.contains((mouse.column, mouse.row).into()))
        }
        _ => false,
    };
    if open {
        c.app.context_help = Some(ContextHelp { topic, scroll: 0 });
    }
    open
}

fn overlay(c: &mut Controller, event: &Event, geometry: &Geometry) {
    let limit = geometry
        .context_help
        .as_ref()
        .map_or(0, |layout| layout.scroll_limit);
    let page = geometry
        .context_help
        .as_ref()
        .map_or(1, |layout| layout.shell.body.height.max(1));
    if let Event::Resize(width, height) = event {
        let geometry =
            crate::ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, *width, *height));
        let limit = geometry
            .context_help
            .map_or(0, |layout| layout.scroll_limit);
        let state = c.app.context_help.as_mut().expect("context layer");
        state.scroll = state.scroll.min(limit);
        return;
    }
    if matches!(event, Event::Key(key) if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc)
        || matches!(event, Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) && geometry.context_help.as_ref().is_some_and(|layout| layout.shell.hit(mouse.column, mouse.row).is_some()))
    {
        c.app.context_help = None;
        return;
    }
    let delta = scroll_delta(event, geometry, page);
    let state = c.app.context_help.as_mut().expect("context layer");
    state.scroll = state
        .scroll
        .min(limit)
        .saturating_add_signed(delta.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16)
        .min(limit);
}

const PATTERN: &[&str] = &[
    "Mark, unmark or replace marks on visible entries in the originating pane. Hidden and filtered entries stay unchanged.",
    "Wildcard matches a whole filename: * matches a sequence; ? matches one byte. Regex searches within filenames; ^ and $ anchor it. Case sensitive controls matching in both modes.",
    "Type a literal ? in Pattern. While typing, Alt+w Wildcard, Alt+x Regex and Alt+s Case sensitive change settings without moving focus.",
    "Outside input: w Wildcard, x Regex, s Case sensitive. Use Tab / Shift+Tab or click to choose a field or action; Space activates the focused control.",
    "m Mark adds matches; u Unmark removes matches; r Replace replaces visible marks. While typing use Alt+m / Alt+u / Alt+r. Enter in Pattern moves to an eligible action; a second Enter applies it. Invalid or overlong expressions cannot apply.",
    "Esc cancels the form. Help never applies a pattern or edits its field.",
];
const TRANSFER: &[&str] = &[
    "Copy or move the frozen visible selection to the reviewed destination. Edit the destination field; Ctrl+a clears it, Backspace edits, Enter runs and Esc cancels.",
    "F2 toggles SHA-256 verification. Review source, destination and selection before running. Existing entries use a separate conflict decision; Help does not approve or submit a transfer.",
];
const CREATE: &[&str] = &[
    "Create a new empty file or directory in the shown parent. Type one name; Ctrl+a clears the field and Backspace edits. Existing entries are never overwritten.",
    "Enter creates only when the name is valid and its context is visible. Esc cancels without creating anything. Click the field or controls to select them.",
];
const RENAME: &[&str] = &[
    "Rename the selected entry in its current directory. Type the new name; Ctrl+a clears the field. Enter submits and Esc cancels.",
    "Review the exact native source and destination. An existing different entry is never overwritten. Escaped non-UTF8 names are display-only until you enter a replacement.",
];
const TRASH: &[&str] = &[
    "Review the recursive preview and exact visible selection. Confirmation stays disabled while counting, after failure or when required context is clipped.",
    "Only uppercase T confirms native Trash. Lowercase t does not confirm. Enter and Esc cancel. Help does not change preview readiness or authorise deletion.",
];
const DELETE: &[&str] = &[
    "Permanent deletion cannot be undone. Review the recursive preview and exact selection; required context and operands must be visible before confirmation.",
    "Only uppercase D confirms permanent deletion. Lowercase d does not confirm. Enter and Esc cancel. Help never authorises an operation.",
];
const ATTRIBUTES: &[&str] = &[
    "Edit supported permissions or the read-only attribute for the shown selection. Links and unsupported objects are refused. Ctrl+a clears the field; Enter runs and Esc cancels.",
];
const CONFLICT: &[&str] = &[
    "Compare Incoming (source) with Existing (destination). Arrows choose a permitted action; Enter confirms the selection. Disabled actions stay unavailable.",
    "s Skip, r Replace, k Keep both, n Newer (only when reliably available). a Apply to all toggles the modifier for subsequent compatible conflicts; Esc cancels the job. Lowercase and uppercase action letters are interchangeable.",
    "Newer keeps Existing unless Incoming has a later verified modification time. Help does not send a conflict decision or change Apply to all.",
];
const JOBS: &[&str] = &[
    "Select a job with Up/Down or a click. e Evidence switches between the compact summary and full per-entry details, receipts and recovery paths.",
    "c Cancel requests safe cancellation; r Retry retries eligible incomplete work; x Reconcile checks eligible uncertain outcomes. These actions depend on the selected job's actual state. Enter opens a pending conflict; Esc closes Jobs.",
    "PageUp / PageDown and the mouse wheel scroll the details. Lowercase and uppercase action letters are interchangeable. Help preserves the selected job, summary/evidence mode and scroll.",
];
const EXIT: &[&str] = &[
    "Active jobs must finish or acknowledge cancellation before exit. r Return (also Enter or Esc), w Wait then exit, c Cancel safely then exit. Lowercase and uppercase letters are interchangeable.",
    "Help never quits, saves a session or requests cancellation. Closing Help restores the safe-exit choice.",
];
const INCIDENT: &[&str] = &[
    "The operating system reported a critical Trash incident. Review all evidence and recovery information. Exit remains blocked while jobs are active or outcomes are unknown.",
    "Type ACKNOWLEDGE exactly, then use F10 only when enabled. Enter and Esc stay in the application. Help never types acknowledgement or authorises exit.",
];

fn scroll_delta(event: &Event, geometry: &Geometry, page: u16) -> i32 {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release && key.modifiers.is_empty() => {
            match key.code {
                KeyCode::Up => -1,
                KeyCode::Down => 1,
                KeyCode::PageUp => -(page as i32),
                KeyCode::PageDown => i32::from(page),
                KeyCode::Home => -i32::MAX,
                KeyCode::End => i32::MAX,
                _ => 0,
            }
        }
        Event::Mouse(mouse)
            if geometry.context_help.as_ref().is_some_and(|layout| {
                layout.shell.body.contains((mouse.column, mouse.row).into())
            }) =>
        {
            match mouse.kind {
                MouseEventKind::ScrollUp => -1,
                MouseEventKind::ScrollDown => 1,
                _ => 0,
            }
        }
        _ => 0,
    }
}

fn bound(app: &App, action: Action) -> &str {
    app.help_rows
        .iter()
        .find(|entry| entry.action == action)
        .map_or("Unbound", |entry| entry.keys.as_str())
}

fn metadata_text(app: &App) -> Vec<String> {
    vec![
        "Inspect the selected entry without changing files. Click attributes, timestamps or owner/group to change their temporary presentation.".into(),
        format!("{} counts directory descendants on demand. {} cancels an active count or closes the floating popup under its existing rules.", bound(app, Action::Count), bound(app, Action::Cancel)),
        format!("Scroll: {} / {}; {} / {}; {} / {}, or the mouse wheel.", bound(app, Action::Up), bound(app, Action::Down), bound(app, Action::Home), bound(app, Action::End), bound(app, Action::PageUp), bound(app, Action::PageDown)),
        format!("Browser shortcut: {}. Help preserves the popup and its scroll. Close Help first, then close the popup before navigating or starting a file operation.", bound(app, Action::Metadata)),
    ]
}
