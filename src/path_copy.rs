//! Local native-path clipboard consumer. Remote references require their own typed consumer.
mod base;
mod directory;
mod request;
use directory::Directory;
pub use request::{BaseRequest, CopyRequest, ValidatedBase};
mod focus;
mod paths;
pub(crate) use focus::listing_focus;
/// Detailed Help opens with `F1` or `?` during Copy Path, including at narrow widths.
pub const HELP: &[&str] = &[
    "Press `F1` or `?` during Copy Path for Help. `Esc` or ‘[×]’ closes Help; press `Esc` again to cancel Copy Path.",
    "Choose `m` ‘Marked’, `h` ‘Highlighted’ or `c` ‘Current folder’, then `n` ‘Name’, `r` ‘Relative’ or `f` ‘Full path’. With the default binding, `y h f` copies the highlighted path.",
    "`Enter` accepts the highlighted choice. ‘From’ defaults to visible marks, then a real highlighted entry, then the current folder. ‘Format’ defaults to ‘Full path’. The .. row is navigation only; ‘Current folder’ means the directory you are browsing.",
    "In ‘From’, marked and highlighted entries keep their normal appearance. After choosing ‘Marked’ or ‘Highlighted’, only that source stays bright. Other entries in both panes dim until you finish or cancel; entry errors stay visible.",
    "Press a choice's letter or click it to continue. `Backspace` returns to ‘From’, or cancels if you are there. `Esc` cancels at either stage. Arrow keys and `Tab` leave the choice unchanged. There is no timeout.",
    "‘From’ also offers `s` ‘Set base’ and `Shift+s` ‘Other base’. Choose one to use the active or other pane's directory as the relative-path base. Wait for confirmation; this exits without copying. ‘Other base’ requires a valid directory in two-pane mode. ‘Relative’ copies from this session base; paths on incomparable roots are disabled.",
    "The initial base is the first directory argument, or the working directory if none is given. Restoring a session restores its base; older sessions use the first pane. Save and quit or --save-session saves the base; ordinary quit does not. The base belongs to the navigation session, not configuration.",
    "The ‘Relative’ choice requires an accessible directory as its base. If the base was deleted, became unreadable or is no longer a directory, copying fails and leaves the clipboard and base unchanged. Choose ‘Set base’ or ‘Other base’ to replace it. ‘Name’ and ‘Full path’ remain available.",
    "Wait for a valid directory listing before starting Copy Path. After setting a base, wait for confirmation before copying relative paths.",
    "Copy Path consumes `Ctrl+c`, `F5` and `F6` while open. More than 1,024 recorded inputs cancels the workflow with an error; start it again to continue.",
    "Copy Path uses visible marks from the active pane in display order, one path per line (LF). Directories copy their own path without listing their contents. Text is literal and unquoted. Non-Unicode paths or paths containing control characters fail without changing the clipboard.",
    "Local clipboard support: wl-copy on Wayland, xclip/xsel on X11, pbcopy on macOS and the native Windows clipboard. Clipboard commands time out after three seconds. Text limit: 16 MiB.",
    "Over SSH, Copy Path sends OSC 52 to the terminal-side clipboard. For local OSC 52 fallback, set clipboard.osc52_fallback = true. The terminal may refuse the text even after tfm sends it. Limit: 74,991 UTF-8 bytes. Zellij uses standard framing; tmux needs set-clipboard on and outer terminal support. GNU screen and dumb/unknown terminals are unsupported.",
];
use crate::{
    app::App,
    fs::{Entry, Identity},
    workflow::{Choice, Completion, Flow, Input, Stage},
};
pub use paths::relative;
use std::{collections::HashMap, path::PathBuf};

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
    directory: Directory,
    highlighted: Option<Entry>,
    marked: Vec<Entry>,
    bases: base::Bases,
}

pub type Workflow = Flow<Snapshot, Completion<Snapshot>>;

pub fn open(app: &App, trigger: Input) -> Result<Workflow, String> {
    let snapshot = Snapshot::capture(app)?;
    let stages = vec![from_stage(&snapshot), formats(&snapshot)];
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

fn from_stage(snapshot: &Snapshot) -> Stage {
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
    let mut choices = vec![
        marked,
        highlighted,
        Choice::new("current", 'c', ["Current folder"; 3]),
    ];
    choices.extend(snapshot.bases.choices());
    Stage::new("From", choices, default)
}

fn formats(snapshot: &Snapshot) -> Stage {
    let mut relative = Choice::new("relative", 'r', ["Relative", "Relative", "Rel"]);
    if let Err(reason) = snapshot.bases.copy_base() {
        relative.disabled = Some(reason);
    }
    Stage::new(
        "Format",
        vec![
            Choice::new("name", 'n', ["Name"; 3]),
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
    fn capture(app: &App) -> Result<Self, String> {
        let pane = &app.panes[app.active];
        if pane.pending.is_some() || pane.error.is_some() || !pane.path.is_absolute() {
            return Err("Wait for a valid local directory before copying paths".into());
        }
        let directory = Directory::capture(pane)?;
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
            highlighted: pane.current().filter(|e| e.identity.is_some()).cloned(),
            marked,
            bases: base::Bases::capture(app),
        })
    }

    fn validate(&self, app: &App, source: Source) -> Result<Vec<Entry>, String> {
        let pane = &app.panes[self.pane];
        self.validate_context(app)?;
        let entries = match source {
            Source::Current => vec![self.directory.entry()],
            Source::Highlighted => self.validate_highlight(pane)?,
            Source::Marked => self.validate_marks(app)?,
        };
        Ok(entries)
    }

    fn validate_context(&self, app: &App) -> Result<(), String> {
        if !self.matches_context(app) {
            return Err("Path-copy context changed; start again".into());
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
        let visible: HashMap<_, _> = pane
            .visible
            .iter()
            .map(|i| (&pane.entries[*i].path, &pane.entries[*i]))
            .collect();
        for entry in &self.marked {
            let visible = visible
                .get(&entry.path)
                .is_some_and(|fresh| same_entry(entry, fresh));
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

/// A completed flow queues frozen work for copying or setting the session base.
pub enum Outcome {
    Copy(CopyRequest),
    BaseSet(BaseRequest),
}

/// Validate current app state and queue frozen native work without filesystem I/O.
pub fn complete(done: Completion<Snapshot>, app: &mut App) -> Result<Outcome, String> {
    let ids: Vec<_> = done.choices.iter().map(|choice| choice.id).collect();
    match ids.as_slice() {
        ["path-copy", base] => {
            done.context.validate_context(app)?;
            let path = done.context.bases.select(base, app)?;
            Ok(Outcome::BaseSet(BaseRequest::new(done.context, base, path)))
        }
        ["path-copy", source, format] => copy_request(
            done.context,
            source_choice(source)?,
            format_choice(format)?,
            app,
        )
        .map(Outcome::Copy),
        _ => Err("Incomplete path-copy choices".into()),
    }
}

fn copy_request(
    snapshot: Snapshot,
    source: Source,
    format: Format,
    app: &App,
) -> Result<CopyRequest, String> {
    let base = if format == Format::Relative {
        Some(snapshot.bases.copy_base()?)
    } else {
        None
    };
    let entries = snapshot.validate(app, source)?;
    let scope = scope(source, format, entries.len());
    Ok(CopyRequest::new(
        entries,
        snapshot.directory,
        source,
        format,
        base.cloned(),
        scope,
    ))
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

#[cfg(test)]
mod tests;
