//! Frozen reference choices; only a completed Relative workflow changes session state.
use crate::{
    app::App,
    workflow::{Choice, Stage},
};
use std::path::PathBuf;

type Candidate = (
    &'static str,
    char,
    [&'static str; 3],
    Result<PathBuf, String>,
);

pub(super) struct Bases {
    candidates: Vec<Candidate>,
    opposite: Option<(usize, PathBuf, u64)>,
}

impl Bases {
    pub(super) fn capture(app: &App) -> Self {
        let pane = &app.panes[app.active];
        let other = &app.panes[1 - app.active];
        let opposite = app
            .dual
            .then(|| (1 - app.active, other.path.clone(), other.context));
        let other_base = if app.dual && other.pending.is_none() && other.error.is_none() {
            Ok(other.path.clone())
        } else {
            Err("Other pane base requires a valid directory in logical two-pane mode".into())
        };
        let mut candidates = vec![
            (
                "keep-base",
                'k',
                ["Keep base", "Keep", "Keep"],
                Ok(app.relative_path_base.clone()),
            ),
            (
                "current-base",
                's',
                ["This pane", "This", "Here"],
                Ok(pane.path.clone()),
            ),
            (
                "opposite-base",
                'S',
                ["Other pane", "Other", "Other"],
                other_base,
            ),
            (
                "startup-base",
                'o',
                ["Startup base", "Startup", "Orig"],
                app.launch_directory
                    .clone()
                    .ok_or("Startup base is unavailable".into()),
            ),
        ];
        for (_, _, _, base) in &mut candidates {
            *base = base
                .clone()
                .and_then(|path| super::relative(&pane.path, &path).map(|_| path));
        }
        Self {
            candidates,
            opposite,
        }
    }

    pub(super) fn stage(&self) -> Stage {
        let choices = self
            .candidates
            .iter()
            .map(|(id, key, labels, base)| {
                let choice = Choice::new(id, *key, *labels);
                match base {
                    Ok(_) => choice,
                    Err(reason) => choice.disabled(reason),
                }
            })
            .collect();
        Stage::new("Base", choices, "keep-base")
    }

    pub(super) fn resolve(
        &self,
        format: super::Format,
        choices: &[&str],
        app: &App,
    ) -> Result<Option<&PathBuf>, String> {
        match (format, choices) {
            (super::Format::Relative, [id]) => self.selected(id, app).map(Some),
            (super::Format::Name | super::Format::Full, []) => Ok(None),
            _ => Err("Incomplete path-copy base choice".into()),
        }
    }

    fn selected(&self, id: &str, app: &App) -> Result<&PathBuf, String> {
        if id == "opposite-base" {
            let (pane, path, context) =
                self.opposite.as_ref().ok_or("Other pane is unavailable")?;
            let fresh = &app.panes[*pane];
            if !app.dual
                || fresh.path != *path
                || fresh.context != *context
                || fresh.pending.is_some()
                || fresh.error.is_some()
            {
                return Err("Other pane reference changed; start again".into());
            }
        }
        self.candidates
            .iter()
            .find(|(candidate, ..)| *candidate == id)
            .ok_or_else(|| "Unknown relative base".to_owned())?
            .3
            .as_ref()
            .map_err(Clone::clone)
    }
}
