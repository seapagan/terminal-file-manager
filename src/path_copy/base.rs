//! Frozen session base and independently validated capture actions.
use super::Directory;
use crate::{app::App, workflow::Choice};
use std::path::PathBuf;

pub(super) struct Bases {
    selected: PathBuf,
    current: PathBuf,
    opposite: Result<Reference, String>,
}

struct Reference {
    pane: usize,
    context: u64,
    directory: Directory,
    path: PathBuf,
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

    pub(super) fn validate_native(
        &self,
        id: &str,
        inspect: &mut impl FnMut(&std::path::Path) -> crate::fs::Entry,
    ) -> Result<(), String> {
        if id == "opposite-base" {
            self.opposite
                .as_ref()
                .map_err(Clone::clone)?
                .directory
                .validate(inspect)?;
        }
        Ok(())
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
        let directory = Directory::capture(pane)?;
        Ok(Self {
            pane: id,
            context: pane.context,
            directory,
            path: pane.path.clone(),
        })
    }

    fn validate(&self, app: &App) -> Result<PathBuf, String> {
        let pane = &app.panes[self.pane];
        if !app.dual
            || pane.path != self.path
            || pane.context != self.context
            || pane.pending.is_some()
            || pane.error.is_some()
        {
            return Err("Other pane reference changed; start again".into());
        }
        Ok(self.path.clone())
    }
}
