//! Frozen session base and independently validated capture actions.
use crate::{app::App, fs::Entry, workflow::Choice};
use std::path::PathBuf;

pub(super) struct Bases {
    selected: PathBuf,
    current: PathBuf,
    opposite: Result<Reference, String>,
}

struct Reference {
    pane: usize,
    context: u64,
    directory: Entry,
}

impl Bases {
    pub(super) fn capture(app: &App) -> Self {
        Self {
            selected: app.relative_path_base.clone(),
            current: app.panes[app.active].path.clone(),
            opposite: Reference::capture(app),
        }
    }

    pub(super) fn choices(&self) -> [Choice; 2] {
        let mut other =
            Choice::new("opposite-base", 'S', ["Other base", "Other base", "Other"]).completes();
        if let Err(reason) = &self.opposite {
            other.disabled = Some(reason.clone());
        }
        [
            Choice::new("current-base", 's', ["Set base", "Set base", "Set"]).completes(),
            other,
        ]
    }

    pub(super) fn copy_base(&self) -> Result<&PathBuf, String> {
        super::relative(&self.current, &self.selected)?;
        Ok(&self.selected)
    }

    pub(super) fn select(&self, id: &str, app: &App) -> Result<PathBuf, String> {
        match id {
            "current-base" => Ok(self.current.clone()),
            "opposite-base" => self.opposite.as_ref().map_err(Clone::clone)?.validate(app),
            _ => Err("Unknown relative-base action".into()),
        }
    }
}

impl Reference {
    fn capture(app: &App) -> Result<Self, String> {
        let id = 1 - app.active;
        let pane = &app.panes[id];
        if !app.dual || pane.pending.is_some() || pane.error.is_some() || !pane.path.is_absolute() {
            return Err("Other base requires a valid directory in logical two-pane mode".into());
        }
        let directory = crate::fs::inspect(&pane.path);
        super::validate_directory(pane, &directory)?;
        Ok(Self {
            pane: id,
            context: pane.context,
            directory,
        })
    }

    fn validate(&self, app: &App) -> Result<PathBuf, String> {
        let pane = &app.panes[self.pane];
        let fresh = crate::fs::inspect(&self.directory.path);
        if !app.dual
            || pane.path != self.directory.path
            || pane.context != self.context
            || pane.pending.is_some()
            || pane.error.is_some()
            || !super::same_entry(&self.directory, &fresh)
        {
            return Err("Other pane reference changed; start again".into());
        }
        super::validate_directory(pane, &fresh)?;
        Ok(self.directory.path.clone())
    }
}
