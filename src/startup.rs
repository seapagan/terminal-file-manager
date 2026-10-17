//! Native path startup options, separate from saved view policy.
use crate::Invocation;
use std::{ffi::OsString, path::PathBuf};

/// Explicit command-line startup intent. No lossy path conversion.
#[derive(Clone, Debug, Default)]
pub struct Startup {
    pub paths: Vec<PathBuf>,
    pub two_panes: bool,
    pub restore_session: bool,
    pub save_session: bool,
}

pub(crate) fn parse(arguments: &[OsString]) -> Result<Invocation, &'static str> {
    let mut startup = Startup::default();
    let mut positional = false;
    for argument in arguments {
        if positional {
            startup.paths.push(argument.into());
            continue;
        }
        match argument.as_encoded_bytes() {
            b"--" => positional = true,
            b"--restore-session" => startup.restore_session = true,
            b"--save-session" => startup.save_session = true,
            b"-2" | b"--two-panes" => startup.two_panes = true,
            bytes if bytes.starts_with(b"-") => return Err("unsupported arguments; use --help"),
            _ => startup.paths.push(argument.into()),
        }
    }
    if startup.paths.len() > 2 {
        return Err("unsupported arguments; use --help");
    }
    if startup.restore_session && !startup.paths.is_empty() {
        return Err("--restore-session cannot be combined with explicit paths");
    }
    Ok(Invocation::BrowseOptions(startup))
}

impl Startup {
    /// Duplicate the one explicit location; two explicit locations imply dual panes.
    pub fn locations(&self) -> [PathBuf; 2] {
        let left = self.paths.first().cloned().unwrap_or_else(|| ".".into());
        let right = self.paths.get(1).cloned().unwrap_or_else(|| left.clone());
        [left, right]
    }
}

/// Default navigation storage lives beside preferences, but has a separate schema.
///
/// # Errors
/// Reports a missing user configuration location rather than choosing another directory.
pub fn session_store() -> Result<crate::session::SessionStore, String> {
    let path = crate::settings::config_path()
        .ok_or("no user config directory is available for navigation sessions")?
        .with_file_name("session.toml");
    Ok(crate::session::SessionStore { path })
}

impl Startup {
    fn restoring(&self, settings: &crate::settings::Settings) -> bool {
        self.restore_session || (settings.restore_session && self.paths.is_empty())
    }

    pub(crate) fn prepare(
        &self,
        settings: &crate::settings::Settings,
    ) -> Result<crate::app::App, String> {
        if self.paths.len() > 2 {
            return Err("at most two directory paths are supported".into());
        }
        let restore = self.restoring(settings);
        let mut app = if restore {
            session_store()?.load()?.restore(settings)?
        } else {
            let [left, right] = self.locations();
            let mut app = crate::app::App::new(left);
            app.panes[1].path = right;
            settings.apply(&mut app);
            app
        };
        for pane in &mut app.panes {
            pane.path = validate_path(&pane.path)?;
        }
        if self.two_panes || self.paths.len() == 2 {
            app.dual = true;
            app.maximized = false;
        }
        Ok(app)
    }
}

fn validate_path(path: &std::path::Path) -> Result<PathBuf, String> {
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    std::fs::read_dir(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}
