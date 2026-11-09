//! Local native-path clipboard consumer. Remote references require their own typed consumer.
mod paths;
/// Detailed Help remains available before activation, including at narrow widths.
pub const HELP: &[&str] = &[
    "Choose m Marked, h Highlighted or c Current directory, then n Name, r Relative or f Full path. Example: y h f copies the highlighted path with the default binding.",
    "Enter accepts the highlighted choice: visible marks first, then the real highlighted entry, then current directory. Full path starts selected. The synthetic .. row is never Highlighted; Current still means this directory.",
    "Up/Down/Right and Tab/Shift+Tab change the choice. Left/Backspace goes back; Esc cancels. Click a displayed choice to activate it. Unsupported keys stay inside the workflow; there is no timeout.",
    "Relative uses the original launch directory, including after navigation. Configure clipboard.relative_path_base = opposite-pane to use the other pane instead; logical two-pane mode and comparable roots are required, even when a pane is hidden or maximised.",
    "Only visible marks in the active pane are copied, in displayed order, one per LF-delimited line. Directories copy their own path, without traversal. Text is literal and unquoted. Non-Unicode or control-containing paths are rejected without changing the clipboard.",
];
use crate::{
    app::App,
    fs::{Entry, Identity},
    workflow::{Choice, Completion, Flow, Input, Stage},
};
pub use paths::relative;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelativeBase {
    #[default]
    Launch,
    OppositePane,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub relative_path_base: RelativeBase,
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
    highlighted: Option<Entry>,
    marked: Vec<Entry>,
    base: Result<PathBuf, String>,
    opposite: Option<(usize, PathBuf, u64)>,
}

pub type Workflow = Flow<Snapshot, Completion<Snapshot>>;

/// Fully validated batch, ready for one asynchronous clipboard write.
#[derive(Debug)]
pub struct CopyRequest {
    pub text: String,
    pub scope: String,
}

pub fn open(app: &App, preferences: &Preferences, trigger: Input) -> Result<Workflow, String> {
    let snapshot = Snapshot::capture(app, preferences)?;
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
            "Source",
            vec![
                marked,
                highlighted,
                Choice::new("current", 'c', ["Current directory", "Current", "Here"]),
            ],
            default,
        ),
        formats(&snapshot),
    ];
    Flow::new("path-copy", "YANK", trigger, snapshot, stages, |done| done).map_err(str::to_owned)
}

fn formats(snapshot: &Snapshot) -> Stage {
    let mut relative = Choice::new("relative", 'r', ["Relative", "Relative", "Rel"]);
    if let Err(reason) = &snapshot.base {
        relative.disabled = Some(reason.clone());
    }
    Stage::new(
        "Format",
        vec![
            Choice::new("name", 'n', ["Name", "Name", "Name"]),
            relative,
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
            && pane.context == self.context
            && pane.pending.is_none()
            && pane.error.is_none()
    }
    fn capture(app: &App, preferences: &Preferences) -> Result<Self, String> {
        let pane = &app.panes[app.active];
        if pane.pending.is_some() || pane.error.is_some() || !pane.path.is_absolute() {
            return Err("Wait for a valid local directory before copying paths".into());
        }
        let directory = crate::fs::inspect(&pane.path);
        if directory.identity.is_none() {
            return Err("Current directory identity is unavailable; refresh first".into());
        }
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
        let opposite = (preferences.relative_path_base == RelativeBase::OppositePane && app.dual)
            .then(|| {
                let id = 1 - app.active;
                (id, app.panes[id].path.clone(), app.panes[id].context)
            });
        let base = match preferences.relative_path_base {
            RelativeBase::Launch => app
                .launch_directory
                .clone()
                .ok_or_else(|| "Launch directory is unavailable".into()),
            RelativeBase::OppositePane => opposite
                .as_ref()
                .map(|(_, path, _)| path.clone())
                .ok_or_else(|| "Relative requires logical two-pane mode; enable two panes".into()),
        }
        .and_then(|base| relative(&pane.path, &base).map(|_| base));
        Ok(Self {
            pane: app.active,
            path: pane.path.clone(),
            context: pane.context,
            directory,
            highlighted: pane.current().filter(|e| e.identity.is_some()).cloned(),
            marked,
            base,
            opposite,
        })
    }

    fn validate(&self, app: &App, source: Source, format: Format) -> Result<Vec<Entry>, String> {
        let pane = &app.panes[self.pane];
        if !self.matches_context(app)
            || !same_entry(&self.directory, &crate::fs::inspect(&self.path))
        {
            return Err("Path-copy context changed; start again".into());
        }
        if format == Format::Relative {
            self.validate_base(app)?;
        }
        let entries = match source {
            Source::Current => vec![self.directory.clone()],
            Source::Highlighted => self.validate_highlight(pane)?,
            Source::Marked => self.validate_marks(app)?,
        };
        validate_entries(&entries)?;
        Ok(entries)
    }

    fn validate_highlight(&self, pane: &crate::app::Pane) -> Result<Vec<Entry>, String> {
        let entry = self.highlighted.as_ref().ok_or("No highlighted entry")?;
        if !pane.current().is_some_and(|fresh| same_entry(entry, fresh)) {
            return Err("Highlighted entry changed; start again".into());
        }
        Ok(vec![entry.clone()])
    }

    fn validate_base(&self, app: &App) -> Result<(), String> {
        self.base.as_ref().map_err(Clone::clone)?;
        if let Some((id, path, context)) = &self.opposite {
            let pane = &app.panes[*id];
            if !app.dual
                || pane.path != *path
                || pane.context != *context
                || pane.pending.is_some()
                || pane.error.is_some()
            {
                return Err("Opposite pane reference changed; start again".into());
            }
        }
        Ok(())
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

/// Resolve semantic IDs, validate the complete batch, then construct exact clipboard text.
pub fn complete(done: Completion<Snapshot>, app: &App) -> Result<CopyRequest, String> {
    let ids: Vec<_> = done.choices.iter().map(|c| c.id).collect();
    let ["path-copy", source, format] = ids.as_slice() else {
        return Err("Incomplete path-copy choices".into());
    };
    let source = source_choice(source)?;
    let format = format_choice(format)?;
    let entries = done.context.validate(app, source, format)?;
    let values = entries
        .iter()
        .map(|entry| {
            paths::text(
                &SourceIdentity::Local(entry.clone()),
                format,
                done.context.base.as_ref().ok().map(PathBuf::as_path),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let text = values.join("\n");
    if text.len() > paths::MAX_BYTES {
        return Err("Path text exceeds the 16 MiB clipboard limit; copy fewer entries".into());
    }
    let scope = scope(source, format, entries.len());
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
