//! Context prose identifies shortcut fragments before any theme or wrapping.
use super::HelpTopic;
use crate::{app::App, input::Action, operations::Operation};

/// A prose fragment or literal shortcut, styled only by the Help renderer.
#[derive(Clone, Debug)]
pub enum HelpFragment {
    Text(String),
    Key(String),
}
/// One paragraph before display-cell measurement.
pub type HelpParagraph = Vec<HelpFragment>;

// Backticks mark fixed keys; {} inserts one effective binding without parsing it.
fn paragraph(text: &str, bindings: &[&str]) -> HelpParagraph {
    let mut bindings = bindings.iter();
    text.split('`')
        .enumerate()
        .filter(|(_, part)| !part.is_empty())
        .map(|(index, part)| {
            if index % 2 == 0 {
                return HelpFragment::Text(part.into());
            }
            let key = if part == "{}" {
                *bindings.next().expect("shortcut placeholder")
            } else {
                part
            };
            if key == "Unbound" {
                HelpFragment::Text("Not bound".into())
            } else {
                HelpFragment::Key(key.into())
            }
        })
        .collect()
}

pub(super) fn text(topic: HelpTopic, app: &App) -> Vec<HelpParagraph> {
    let (action, lines) = match topic {
        HelpTopic::Pattern => (Action::PatternSelection, PATTERN),
        HelpTopic::Conflict => (Action::Jobs, CONFLICT),
        HelpTopic::Jobs => (Action::Jobs, JOBS),
        HelpTopic::Metadata => return metadata(app),
        HelpTopic::Exit => (Action::Quit, EXIT),
        HelpTopic::Incident => (Action::Quit, INCIDENT),
        HelpTopic::Operation(operation) => operation_text(operation),
    };
    let mut text: Vec<_> = lines.iter().map(|line| paragraph(line, &[])).collect();
    if let Some(entry) = app.help_rows.iter().find(|entry| entry.action == action) {
        text.push(paragraph("Open from the file list: `{}`.", &[&entry.keys]));
    }
    text
}

fn operation_text(operation: Operation) -> (Action, &'static [&'static str]) {
    match operation {
        Operation::Copy => (Action::Copy, TRANSFER),
        Operation::Move => (Action::Move, TRANSFER),
        Operation::Mkdir => (Action::Mkdir, CREATE),
        Operation::CreateFile => (Action::CreateFile, CREATE),
        Operation::Rename => (Action::Rename, RENAME),
        Operation::Trash => (Action::Trash, TRASH),
        Operation::Delete => (Action::Delete, DELETE),
        Operation::Permissions(_) | Operation::ReadOnly(_) => (Action::Attributes, ATTRIBUTES),
    }
}

const PATTERN: &[&str] = &[
    "Change marks on entries shown in this pane. Hidden and filtered-out entries keep their marks.",
    "Wildcards match the whole filename: * matches a sequence; ? matches one byte. Regex searches names; ^ and $ anchor the match. Case sensitive applies to both modes.",
    "You can type ? in Pattern. While typing, use `Alt+w` for Wildcard, `Alt+x` for Regex and `Alt+s` for Case sensitive. These shortcuts keep the cursor in Pattern.",
    "Outside Pattern, use `w`, `x` and `s` for those settings. Press `Tab` / `Shift+Tab` or click to choose a field or action. `Space` activates the focused control.",
    "`m` Mark adds matching entries, `u` Unmark removes them, and `r` Replace replaces the visible marks with matches. While typing, use `Alt+m` / `Alt+u` / `Alt+r`.",
    "`Enter` in Pattern moves to an available action; press `Enter` again to apply it. Correct invalid or overlong expressions before applying. Close Help to return to the form; `Esc` then cancels it without changing marks.",
];
const TRANSFER: &[&str] = &[
    "Copy or move the listed selection to the destination in this form. The selection stays fixed while you edit. `Ctrl+a` clears the destination, `Backspace` edits, `Enter` starts and `Esc` cancels.",
    "`F2` toggles SHA-256 verification. Check the source, destination and selection before starting. If an entry already exists, choose how to handle it in the next prompt. Reading Help does not start the transfer.",
];
const CREATE: &[&str] = &[
    "Create an empty file or directory in the shown parent. Type one name. `Ctrl+a` clears the field and `Backspace` edits. Existing entries are never overwritten.",
    "`Enter` creates when the name is valid and the required details are visible. `Esc` cancels without creating anything. Click the field or a control to focus it.",
];
const RENAME: &[&str] = &[
    "Rename the selected entry in its current directory. Type the new name; `Ctrl+a` clears the field. `Enter` renames and `Esc` cancels.",
    "Check the displayed source and new name. tfm will not overwrite another entry. For a name shown with escaped filename bytes, type the replacement name yourself.",
];
const TRASH: &[&str] = &[
    "Review the selected entries and folder totals. You cannot confirm while tfm is counting, after an error, or when required details do not fit on screen.",
    "Only uppercase `T` confirms the move to Trash. Lowercase `t` does not confirm. `Enter` and `Esc` cancel. Close Help before confirming; reading it does not approve the operation.",
];
const DELETE: &[&str] = &[
    "Permanent deletion cannot be undone. Check the selected entries and folder totals. Counting must finish without errors, and the required details and selected entries must be visible before you can confirm.",
    "Only uppercase `D` confirms permanent deletion. Lowercase `d` does not confirm. `Enter` and `Esc` cancel. Close Help before confirming; reading it does not approve the operation.",
];
const ATTRIBUTES: &[&str] = &[
    "Change supported permissions or the read-only setting for the shown selection. tfm refuses links and unsupported entries. `Ctrl+a` clears the field; `Enter` applies the change and `Esc` cancels.",
];
const CONFLICT: &[&str] = &[
    "Compare Incoming (source) with Existing (destination). Use `Up` / `Down` to choose an available action, then `Enter` to apply it. Disabled actions cannot be used.",
    "`s` Skip, `r` Replace, `k` Keep both, `n` Newer. `a` Apply to all uses your choice for later conflicts of the same kind. `Esc` cancels the job. These letters work in either case.",
    "Newer is available only when tfm can verify both modification times. It keeps Existing unless Incoming is newer. Close Help to return to your choice; Apply to all stays unchanged.",
];
const JOBS: &[&str] = &[
    "Choose a job with `Up` / `Down` or a click. Press `e` Evidence to see entry details, receipts and recovery paths; press it again for the summary.",
    "`c` Cancel requests a safe stop. `r` Retry retries unfinished work when available. `x` Reconcile checks operations whose result is unknown. Available actions depend on the job's state. `Enter` opens a pending file conflict; `Esc` closes Jobs.",
    "Scroll details with `PageUp` / `PageDown` or the mouse wheel. Action letters work in either case. Close Help to return to the same job, details view and scroll position.",
];
const EXIT: &[&str] = &[
    "Wait for jobs to finish or stop safely before you quit. `r` Return (also `Enter` or `Esc`), `w` Wait then exit, `c` Cancel safely then exit. These letters work in either case.",
    "Close Help to return to this choice. Reading Help does not quit, save a session or cancel jobs.",
];
const INCIDENT: &[&str] = &[
    "Your system reported a critical problem during a Trash operation. Check the job details and recovery information. You cannot quit while jobs are active or their results are unknown.",
    "Type ACKNOWLEDGE exactly, then press `F10` when enabled. `Enter` and `Esc` keep tfm open. Close Help to return; reading it does not enter the acknowledgement or approve exit.",
];

fn bound(app: &App, action: Action) -> &str {
    app.help_rows
        .iter()
        .find(|entry| entry.action == action)
        .map_or("Unbound", |entry| entry.keys.as_str())
}
fn metadata(app: &App) -> Vec<HelpParagraph> {
    vec![
        paragraph(
            "Inspect the selected entry without changing files. Click attributes, times or owner/group to change how they are shown.",
            &[],
        ),
        paragraph(
            "Count files and folders inside the selected directory: `{}`.",
            &[bound(app, Action::Count)],
        ),
        paragraph(
            "Stop a running count, or close Info when no count is running: `{}`.",
            &[bound(app, Action::Cancel)],
        ),
        paragraph(
            "Scroll with `{}` / `{}`, `{}` / `{}`, `{}` / `{}`, or the mouse wheel.",
            &[
                bound(app, Action::Up),
                bound(app, Action::Down),
                bound(app, Action::Home),
                bound(app, Action::End),
                bound(app, Action::PageUp),
                bound(app, Action::PageDown),
            ],
        ),
        paragraph(
            "Open Info from the file list: `{}`. Close Help first, then close Info before navigating or starting a file operation. Your Info scroll position stays unchanged.",
            &[bound(app, Action::Metadata)],
        ),
    ]
}
