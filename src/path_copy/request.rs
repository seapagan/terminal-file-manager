//! Frozen work queued by input handling; native I/O and text generation belong to workers.
use super::{Directory, Entry, Format, Snapshot, Source, SourceIdentity, paths};
use crate::app::App;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct CopyRequest {
    pub scope: String,
    entries: Vec<Entry>,
    directory: Box<Directory>,
    source: Source,
    format: Format,
    base: Option<PathBuf>,
}

impl CopyRequest {
    pub(super) fn new(
        entries: Vec<Entry>,
        directory: Directory,
        source: Source,
        format: Format,
        base: Option<PathBuf>,
        scope: String,
    ) -> Self {
        Self {
            entries,
            directory: Box::new(directory),
            source,
            format,
            base,
            scope,
        }
    }

    /// Revalidate the entire frozen batch before producing any clipboard text. Run off the UI thread.
    pub fn prepare(self) -> Result<String, String> {
        self.prepare_using(crate::fs::inspect_identity)
    }

    pub(crate) fn prepare_using(
        self,
        mut inspect: impl FnMut(&Path) -> Entry,
    ) -> Result<String, String> {
        self.validate(&mut inspect)?;
        let mut text = String::new();
        for entry in self.entries {
            let value = paths::text(
                &SourceIdentity::Local(entry),
                self.format,
                self.base.as_deref(),
            )?;
            if !text.is_empty() {
                text.push('\n');
            }
            if text.len().saturating_add(value.len()) > crate::clipboard::MAX_BYTES {
                return Err(
                    "Path text exceeds the 16 MiB clipboard limit; copy fewer entries".into(),
                );
            }
            text.push_str(&value);
        }
        Ok(text)
    }

    fn validate(&self, inspect: &mut impl FnMut(&Path) -> Entry) -> Result<(), String> {
        self.directory.validate(inspect)?;
        if self.entries.is_empty()
            || (self.source != Source::Current
                && self.entries.iter().any(|entry| {
                    let fresh = inspect(&entry.path);
                    fresh.error.is_some() || !super::same_entry(entry, &fresh)
                }))
        {
            return Err("Selected path was removed or replaced; refresh and start again".into());
        }
        self.validate_base()
    }

    fn validate_base(&self) -> Result<(), String> {
        if self.format == Format::Relative {
            let base = self
                .base
                .as_ref()
                .ok_or("Relative reference is unavailable")?;
            std::fs::read_dir(base).map_err(|error| format!(
                "Cannot read relative-path base '{}': {error}. Choose an accessible directory with Set base or Other base; selected base unchanged",
                base.display()
            ))?;
        }
        Ok(())
    }
}

pub struct BaseRequest {
    snapshot: Box<Snapshot>,
    id: &'static str,
    path: PathBuf,
}

impl BaseRequest {
    pub(super) fn new(snapshot: Snapshot, id: &'static str, path: PathBuf) -> Self {
        Self {
            snapshot: Box::new(snapshot),
            id,
            path,
        }
    }

    /// Validate captured native directories off the UI thread before applying the result.
    pub fn prepare(self) -> Result<ValidatedBase, String> {
        self.prepare_using(crate::fs::inspect_identity)
    }

    pub(crate) fn prepare_using(
        self,
        mut inspect: impl FnMut(&Path) -> Entry,
    ) -> Result<ValidatedBase, String> {
        self.snapshot.directory.validate(&mut inspect)?;
        self.snapshot.bases.validate_native(self.id, &mut inspect)?;
        Ok(ValidatedBase(self))
    }
}

/// Native validation has succeeded; only cached app-state checks remain.
pub struct ValidatedBase(BaseRequest);

impl ValidatedBase {
    /// Recheck app context when a validated worker result arrives; never retarget a captured base.
    pub fn apply(self, app: &mut App) -> Result<String, String> {
        let request = self.0;
        request.snapshot.validate_context(app)?;
        let path = request.snapshot.bases.select(request.id, app)?;
        if path != request.path {
            return Err("Relative-path base context changed; start again".into());
        }
        app.relative_path_base = path;
        Ok(format!(
            "Relative-path base set: {}",
            request.path.display()
        ))
    }
}
