//! Local native-path clipboard consumer. Remote references require their own typed consumer.
mod base;
mod paths;
/// Detailed Help remains available before activation, including at narrow widths.
pub const HELP: &[&str] = &[
    "Choose m Marked, h Highlighted or c Current directory, then n Name, r Relative or f Full path. Example: y h f copies the highlighted path with the default binding.",
    "Enter accepts the highlighted choice: visible marks first, then the real highlighted entry, then current directory. Full path starts selected. The synthetic .. row is never Highlighted; Current still means this directory.",
    "Copy Path starts with From: choose a direct letter shortcut or press Enter for the highlighted default. Backspace returns to the previous stage; Esc cancels. Click a displayed choice to activate it. Arrow and Tab keys stay inside the workflow without changing the choice; there is no timeout.",
    "Relative opens a Base choice: k Keep base (the default), s This pane, Shift+s Other pane, or o Startup base. Captured bases stay fixed across pane navigation. Other pane requires logical two-pane mode; incomparable roots are disabled.",
    "The startup base is the first explicit CLI directory, or the process working directory when no directory is supplied. Saved navigation sessions restore their captured base; older sessions use the first pane. Shift+q and --save-session save it; ordinary quit does not. It is not a configuration preference.",
    "Only visible marks in the active pane are copied, in displayed order, one per LF-delimited line. Directories copy their own path, without traversal. Text is literal and unquoted. Non-Unicode or control-containing paths are rejected without changing the clipboard.",
    "Local providers: wl-copy on Wayland, xclip/xsel on X11, pbcopy on macOS and native Windows clipboard. Commands have a three-second deadline. Clipboard text is limited to 16 MiB.",
    "SSH sends OSC 52 to the terminal-side clipboard. Local OSC 52 fallback requires clipboard.osc52_fallback = true. Sent does not mean accepted; the terminal may refuse it. Limit: 74,991 UTF-8 bytes. Zellij uses standard framing; tmux needs set-clipboard on and outer terminal support. GNU screen and dumb/unknown terminals are unsupported.",
];
use crate::{
    app::App,
    fs::{Entry, Identity},
    workflow::{Choice, Completion, Flow, Input, Stage},
};
pub use paths::relative;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub osc52_fallback: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    Marked,
    Highlighted,
    Current,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Name,
    Relative,
    Full,
}

/// Explicitly local identity; future remote consumers cannot reinterpret this as a host reference.
pub enum SourceIdentity {
    Local(Entry),
}

/// Frozen native context; generic workflow internals never inspect these fields.
pub struct Snapshot {
    pane: usize,
    path: PathBuf,
    context: u64,
    directory: Entry,
    physical_directory: Option<Entry>,
    highlighted: Option<Entry>,
    marked: Vec<Entry>,
    bases: base::Bases,
}

pub type Workflow = Flow<Snapshot, Completion<Snapshot>>;

/// Fully validated batch, ready for one asynchronous clipboard write.
#[derive(Debug)]
pub struct CopyRequest {
    pub text: String,
    pub scope: String,
}

pub fn open(app: &App, trigger: Input) -> Result<Workflow, String> {
    let snapshot = Snapshot::capture(app)?;
    let mut marked = Choice::new("marked", 'm', ["Marked", "Marked", "Marks"]);
    marked.labels[0] = format!("Marked ({})", snapshot.marked.len());
    if snapshot.marked.is_empty() {
        marked.disabled = Some("No eligible visible marks in this pane".into());
    }
    let mut highlighted = Choice::new("highlighted", 'h', ["Highlighted", "Highlighted", "Entry"]);
    if snapshot.highlighted.is_none() {
        highlighted.disabled = Some("Highlight a real entry; .. is navigation only".into());
    }
    let default = if !snapshot.marked.is_empty() {
        "marked"
    } else if snapshot.highlighted.is_some() {
        "highlighted"
    } else {
        "current"
    };
    let stages = vec![
        Stage::new(
            "From",
            vec![
                marked,
                highlighted,
                Choice::new("current", 'c', ["Current directory", "Current", "Here"]),
            ],
            default,
        ),
        formats(),
        snapshot
            .bases
            .stage()
            .when(|choices| choices.last().is_some_and(|c| c.id == "relative")),
    ];
    Flow::new(
        "path-copy",
        "Copy Path",
        trigger,
        snapshot,
        stages,
        |done| done,
    )
    .map_err(str::to_owned)
}

fn formats() -> Stage {
    Stage::new(
        "Format",
        vec![
            Choice::new("name", 'n', ["Name"; 3]),
            Choice::new("relative", 'r', ["Relative", "Relative", "Rel"]),
            Choice::new("full", 'f', ["Full path", "Full", "Full"]),
        ],
        "full",
    )
}

impl Snapshot {
    pub(crate) fn matches_context(&self, app: &App) -> bool {
        let pane = &app.panes[self.pane];
        app.active == self.pane
            && pane.path == self.path
            && (pane.context == self.context || (self.context == 0 && pane.context == 1))
            && pane.pending.is_none()
            && pane.error.is_none()
    }
    fn capture(app: &App) -> Result<Self, String> {
        let pane = &app.panes[app.active];
        if pane.pending.is_some() || pane.error.is_some() || !pane.path.is_absolute() {
            return Err("Wait for a valid local directory before copying paths".into());
        }
        let directory = crate::fs::inspect(&pane.path);
        validate_directory(pane, &directory)?;
        let marked = pane
            .visible
            .iter()
            .map(|i| &pane.entries[*i])
            .filter(|e| {
                e.identity
                    .as_ref()
                    .zip(pane.marks.get(&e.path))
                    .is_some_and(|(fresh, marked)| same_identity(fresh, marked))
            })
            .cloned()
            .collect();
        Ok(Self {
            pane: app.active,
            path: pane.path.clone(),
            context: pane.context,
            directory,
            physical_directory: pane.directory.clone(),
            highlighted: pane.current().filter(|e| e.identity.is_some()).cloned(),
            marked,
            bases: base::Bases::capture(app),
        })
    }

    fn validate(&self, app: &App, source: Source) -> Result<Vec<Entry>, String> {
        let pane = &app.panes[self.pane];
        self.validate_context(app)?;
        let entries = match source {
            Source::Current => vec![self.directory.clone()],
            Source::Highlighted => self.validate_highlight(pane)?,
            Source::Marked => self.validate_marks(app)?,
        };
        validate_entries(&entries)?;
        Ok(entries)
    }

    fn validate_context(&self, app: &App) -> Result<(), String> {
        if !self.matches_context(app)
            || !same_entry(&self.directory, &crate::fs::inspect(&self.path))
        {
            return Err("Path-copy context changed; start again".into());
        }
        if let Some(directory) = &self.physical_directory {
            validate_entries(std::slice::from_ref(directory))?;
        }
        Ok(())
    }

    fn validate_highlight(&self, pane: &crate::app::Pane) -> Result<Vec<Entry>, String> {
        let entry = self.highlighted.as_ref().ok_or("No highlighted entry")?;
        if !pane.current().is_some_and(|fresh| same_entry(entry, fresh)) {
            return Err("Highlighted entry changed; start again".into());
        }
        Ok(vec![entry.clone()])
    }

    fn validate_marks(&self, app: &App) -> Result<Vec<Entry>, String> {
        let pane = &app.panes[self.pane];
        for entry in &self.marked {
            let visible = pane
                .visible
                .iter()
                .any(|i| same_entry(entry, &pane.entries[*i]));
            if !visible
                || !entry
                    .identity
                    .as_ref()
                    .zip(pane.marks.get(&entry.path))
                    .is_some_and(|(a, b)| same_identity(a, b))
            {
                return Err("Visible marked selection changed; start again".into());
            }
        }
        Ok(self.marked.clone())
    }
}

fn validate_entries(entries: &[Entry]) -> Result<(), String> {
    if entries.is_empty()
        || entries
            .iter()
            .any(|e| !same_entry(e, &crate::fs::inspect(&e.path)))
    {
        return Err("Selected path was removed or replaced; refresh and start again".into());
    }
    Ok(())
}

fn validate_directory(pane: &crate::app::Pane, directory: &Entry) -> Result<(), String> {
    if directory.identity.is_none() {
        return Err("Current directory identity is unavailable; refresh first".into());
    }
    if let Some(expected) = &pane.entry_identity
        && !directory
            .identity
            .as_ref()
            .is_some_and(|fresh| same_identity(expected, fresh))
    {
        return Err(
            "Current directory was replaced; refresh or reopen it before copying paths".into(),
        );
    }
    if let Some(physical) = &pane.directory {
        validate_entries(std::slice::from_ref(physical))?;
    }
    Ok(())
}

/// Resolve semantic IDs, validate the complete batch, then construct exact clipboard text.
pub fn complete(done: Completion<Snapshot>, app: &mut App) -> Result<CopyRequest, String> {
    let ids: Vec<_> = done.choices.iter().map(|c| c.id).collect();
    let ["path-copy", source, format, rest @ ..] = ids.as_slice() else {
        return Err("Incomplete path-copy choices".into());
    };
    let source = source_choice(source)?;
    let format = format_choice(format)?;
    let base = done.context.bases.resolve(format, rest, app)?;
    let entries = done.context.validate(app, source)?;
    let values = entries
        .iter()
        .map(|entry| {
            paths::text(
                &SourceIdentity::Local(entry.clone()),
                format,
                base.map(PathBuf::as_path),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let text = values.join("\n");
    if text.len() > paths::MAX_BYTES {
        return Err("Path text exceeds the 16 MiB clipboard limit; copy fewer entries".into());
    }
    let scope = scope(source, format, entries.len());
    if let Some(base) = base {
        app.relative_path_base = base.clone();
    }
    Ok(CopyRequest { text, scope })
}

fn source_choice(source: &str) -> Result<Source, String> {
    Ok(match source {
        "marked" => Source::Marked,
        "highlighted" => Source::Highlighted,
        "current" => Source::Current,
        _ => return Err("Unknown path-copy source".into()),
    })
}

fn format_choice(format: &str) -> Result<Format, String> {
    Ok(match format {
        "name" => Format::Name,
        "relative" => Format::Relative,
        "full" => Format::Full,
        _ => return Err("Unknown path-copy format".into()),
    })
}

fn scope(source: Source, format: Format, count: usize) -> String {
    let format = match format {
        Format::Name => "name",
        Format::Relative => "relative path",
        Format::Full => "full path",
    };
    if source == Source::Current {
        format!("current directory {format}")
    } else {
        format!("{count} {format}{}", if count == 1 { "" } else { "s" })
    }
}

fn same_entry(a: &Entry, b: &Entry) -> bool {
    a.path == b.path
        && a.kind == b.kind
        && a.identity
            .as_ref()
            .zip(b.identity.as_ref())
            .is_some_and(|(a, b)| same_identity(a, b))
}

fn same_identity(a: &Identity, b: &Identity) -> bool {
    match (a.object, b.object) {
        (Some(left), Some(right)) => left == right && a.created == b.created,
        _ => a == b,
    }
}
