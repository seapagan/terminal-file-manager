//! Context prose identifies shortcut fragments before any theme or wrapping.
use super::{HelpTopic, ViewHelpTopic};
use crate::{app::App, input::Action, operations::Operation};

use super::fragments::{HelpParagraph, paragraph};

pub(super) fn text(topic: HelpTopic, app: &App) -> Vec<HelpParagraph> {
    let (action, lines) = match topic {
        HelpTopic::FullPath | HelpTopic::PathCopy => path_text(topic),
        HelpTopic::View(view) => view_text(view),
        HelpTopic::Metadata => return metadata(app),
        HelpTopic::Pattern => (Action::PatternSelection, PATTERN),
        HelpTopic::Policy(action) => return policy(app, action),
        HelpTopic::Conflict => (Action::Jobs, CONFLICT),
        HelpTopic::Jobs => (Action::Jobs, JOBS),
        HelpTopic::Exit => (Action::Quit, EXIT),
        HelpTopic::Incident => (Action::Quit, INCIDENT),
        HelpTopic::Operation(operation) => operation_text(operation),
    };
    action_paragraphs(app, action, lines)
}

fn policy(app: &App, action: Action) -> Vec<HelpParagraph> {
    let mut text = action_paragraphs(
        app,
        crate::input::policy_menu::opening_action(action),
        super::policy_text::text(action),
    );
    if let Some(entry) = app.help_entry(action) {
        text.push(paragraph(
            "Change directly from the file list: `{}`.",
            &[&entry.keys],
        ));
    }
    text
}

fn path_text(topic: HelpTopic) -> (Action, &'static [&'static str]) {
    if topic == HelpTopic::FullPath {
        (Action::FullPath, FULL_PATH)
    } else {
        (Action::PathCopy, crate::path_copy::HELP)
    }
}

fn action_paragraphs(app: &App, action: Action, lines: &[&str]) -> Vec<HelpParagraph> {
    let mut text: Vec<_> = lines.iter().map(|line| paragraph(line, &[])).collect();
    if action == Action::PathCopy {
        text.push(paragraph(
            "Save pane locations and the relative-path base on exit: `{}`.",
            &[bound(app, Action::SaveQuit)],
        ));
    }
    if let Some(entry) = app.help_entry(action) {
        text.push(paragraph("Open from the file list: `{}`.", &[&entry.keys]));
    }
    text
}

fn view_text(view: ViewHelpTopic) -> (Action, &'static [&'static str]) {
    match view {
        ViewHelpTopic::Options => (Action::ViewOptions, VIEW_OPTIONS),
        ViewHelpTopic::Profiles => (Action::ViewProfiles, VIEW_PROFILES),
        ViewHelpTopic::SaveProfile => (Action::ViewProfiles, SAVE_PROFILE),
    }
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

const FULL_PATH: &[&str] = &[
    "Choose a parent directory from the path shown. This changes only the pane that opened the dialog. The current directory starts selected; press `Enter` to keep it.",
    "Use `Left` / `Up` and `Right` / `Down`, or `Tab` / `Shift+Tab`, to choose a segment. `Home` selects the root; `End` selects the current directory.",
    "Press `Enter` to open the selected ancestor, or click its segment. Use the mouse wheel to scroll long paths.",
    "Press `Esc`, click Esc Cancel or click [×] to cancel. Close Help to return to the selected segment and scroll position.",
];

const PATTERN: &[&str] = &[
    "Match filenames in this pane. Mark and Unmark keep hidden and filtered-out marks. Replace clears all marks in this pane before marking visible matches, even if nothing matches. The other pane keeps its marks. Check the status area for the visible marked count.",
    "Wildcards match the whole filename: * matches a sequence; ? matches one byte. Regex searches names; ^ and $ anchor the match. Case sensitive applies to both modes.",
    "While typing in Pattern, use `Alt+w` for Wildcard, `Alt+x` for Regex and `Alt+s` for Case sensitive. Type ? as part of the pattern; use `F1` for Help.",
    "Outside Pattern, use `w`, `x` and `s` for those settings. Press `Tab` / `Shift+Tab` or click to choose a field or action. `Space` activates the focused control.",
    "`m` Mark adds matching entries, `u` Unmark removes them, and `r` Replace clears all pane marks and marks the visible matches. While typing, use `Alt+m` / `Alt+u` / `Alt+r`.",
    "`Enter` in Pattern moves to an available action; press `Enter` again to apply it. Correct invalid or overlong expressions before applying. `Esc` cancels the form without changing marks.",
];
const TRANSFER: &[&str] = &[
    "Copy or move the listed selection to the destination you enter. The selection stays fixed while you edit. `Ctrl+a` clears the destination, `Backspace` edits, `Enter` starts and `Esc` cancels.",
    "`F2` toggles SHA-256 verification. Check the source, destination and selection before starting. If an entry already exists, choose how to handle it in the conflict prompt.",
];
const CREATE: &[&str] = &[
    "Create an empty file or directory in the shown parent. Type one name. `Ctrl+a` clears the field and `Backspace` edits. Existing entries are never overwritten.",
    "Press `Enter` to create once the name is valid and the required details fit on screen. `Esc` cancels. Click the field or a control to focus it.",
];
const RENAME: &[&str] = &[
    "Rename the selected entry in its current directory. Type the new name; `Ctrl+a` clears the field. `Enter` renames and `Esc` cancels.",
    "Check the source and new name. tfm will not overwrite another entry. If the original name contains escaped filename bytes, type the replacement name.",
];
const TRASH: &[&str] = &[
    "Review the selected entries and folder totals. You cannot confirm while tfm is counting, after an error, or when required details do not fit on screen.",
    "Only uppercase `T` confirms the move to Trash. Lowercase `t` does not confirm. `Enter` and `Esc` cancel. Close Help before confirming.",
];
const DELETE: &[&str] = &[
    "Permanent deletion cannot be undone. Check the selected entries and folder totals. Counting must finish without errors, and the required details and selected entries must be visible before you can confirm.",
    "Only uppercase `D` confirms permanent deletion. Lowercase `d` does not confirm. `Enter` and `Esc` cancel. Close Help before confirming.",
];
const ATTRIBUTES: &[&str] = &[
    "Change supported permissions or the read-only setting for the shown selection. tfm refuses links and unsupported entries. `Ctrl+a` clears the field; `Enter` applies the change and `Esc` cancels.",
];
const CONFLICT: &[&str] = &[
    "Compare Incoming (source) with Existing (destination). Use `Up` / `Down` to choose an available action, then `Enter` to apply it.",
    "`s` Skip, `r` Replace, `k` Keep both, `n` Newer. `a` Apply to all uses your choice for later conflicts of the same kind. `Esc` cancels the job. These letters work in either case.",
    "Newer requires both modification times. It keeps Existing unless Incoming is newer. Close Help to return to your choice and Apply to all setting.",
];
const JOBS: &[&str] = &[
    "Choose a job with `Up` / `Down` or a click. Press `e` Evidence to see entry details, receipts and recovery paths; press it again for the summary.",
    "`c` Cancel requests a safe stop. Use `r` Retry for unfinished work, or `x` Reconcile to check an unknown result. Only available actions are enabled. `Enter` opens a pending file conflict; `Esc` closes Jobs.",
    "Scroll details with `PageUp` / `PageDown` or the mouse wheel. Action letters work in either case. Close Help to return to the selected job and scroll position.",
];
const EXIT: &[&str] = &[
    "Wait for jobs to finish or stop safely before you quit. `r` Return (also `Enter` or `Esc`), `w` Wait then exit, `c` Cancel safely then exit. These letters work in either case.",
    "Close Help to return to the exit choices. Choose an action there to quit or stop jobs.",
];
const INCIDENT: &[&str] = &[
    "Your system reported a critical problem during a Trash operation. Check the job details and recovery information. You cannot quit while jobs are active or their results are unknown.",
    "Type ACKNOWLEDGE exactly, then press `F10` when enabled. `Enter` and `Esc` keep tfm open. Close Help before entering the acknowledgement.",
];

fn bound(app: &App, action: Action) -> &str {
    app.help_entry(action)
        .map_or("Unbound", |entry| entry.keys.as_str())
}
fn metadata(app: &App) -> Vec<HelpParagraph> {
    vec![
        paragraph(
            if cfg!(windows) {
                "Inspect the selected entry. Windows owners show account names or SIDs; unresolved names keep the SID. Click attributes, times or owner to change their format."
            } else {
                "Inspect the selected entry. Click attributes, times or owner/group to change their format."
            },
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
            "Open Info from the file list: `{}`. Close Help to return to Info at the same scroll position. Close Info to navigate or start a file operation.",
            &[bound(app, Action::Metadata)],
        ),
    ]
}

const VIEW_OPTIONS: &[&str] = &[
    "Change the active pane's display settings. Use `Up` / `Down` or the mouse wheel to select a setting. Press `Space` / `Enter` or click its row to change it.",
    "Changes take effect at once. `Esc` closes the menu and keeps them. A changed named profile shows Modified; the saved profile stays unchanged.",
];
const VIEW_PROFILES: &[&str] = &[
    "Choose saved display settings with `Up` / `Down`, `Home` / `End` or the mouse wheel. Click a profile to select it. Press `Enter` to apply it to the active pane.",
    "Press `Space` on a profile to select Apply selected to both, then `Enter` or `Space` to apply it. You can also click that row. Profiles change display settings; your pane locations stay the same.",
    "Choose Save active as… to name and save the active pane's settings. Check the status area for confirmation. `Esc` closes the list without applying the selected profile.",
];
const SAVE_PROFILE: &[&str] = &[
    "Type a name for the active pane's display settings. You can include ? in the name; use `F1` for Help. `Backspace` removes the last character.",
    "Press `Enter` to save. Using an existing name replaces that profile. Correct name errors in the form; check the status area for confirmation. Preferences save on exit. Pane locations stay unchanged. `Esc` discards the name and returns to the profile list.",
];
