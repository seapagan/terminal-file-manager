//! Versioned navigation only: native locations and layout, never view profiles/history.
use crate::{app::App, settings::Settings};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "encoding",
    content = "units",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum NativePath {
    Unix(Vec<u8>),
    Windows(Vec<u16>),
}

impl NativePath {
    fn capture(path: &std::path::Path) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Self::Unix(path.as_os_str().as_bytes().to_vec())
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            Self::Windows(path.as_os_str().encode_wide().collect())
        }
    }

    fn decode(&self) -> Result<PathBuf, String> {
        match self {
            #[cfg(unix)]
            Self::Unix(bytes) => {
                use std::os::unix::ffi::OsStringExt;
                Ok(std::ffi::OsString::from_vec(bytes.clone()).into())
            }
            #[cfg(windows)]
            Self::Windows(units) => {
                use std::os::windows::ffi::OsStringExt;
                Ok(std::ffi::OsString::from_wide(units).into())
            }
            _ => Err("session path encoding belongs to another platform".into()),
        }
    }
}

/// Version 1 saves both current locations and layout only.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationSession {
    version: u32,
    locations: [NativePath; 2],
    active: usize,
    dual: bool,
    maximized: bool,
    split_percent: u16,
}

impl NavigationSession {
    fn capture(app: &App) -> Self {
        Self {
            version: 1,
            locations: std::array::from_fn(|i| NativePath::capture(&app.panes[i].path)),
            active: app.active,
            dual: app.dual,
            maximized: app.maximized,
            split_percent: app.split_percent,
        }
    }

    /// Parse and validate without filesystem reads or fallback locations.
    ///
    /// # Errors
    /// Rejects unknown versions, fields, native encodings and invalid layout/paths.
    pub fn parse(text: &str) -> Result<Self, String> {
        let session: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        session.locations()?;
        Ok(session)
    }

    /// Decode native identities without coercing invalid Unicode.
    ///
    /// # Errors
    /// Rejects foreign encodings, relative paths and invalid schema/layout.
    pub fn locations(&self) -> Result<[PathBuf; 2], String> {
        if self.version != 1 || self.active > 1 || !(20..=80).contains(&self.split_percent) {
            return Err("unsupported session version or invalid layout".into());
        }
        let paths = [self.locations[0].decode()?, self.locations[1].decode()?];
        if paths.iter().any(|p| !p.is_absolute()) {
            return Err("session locations must be absolute native paths".into());
        }
        Ok(paths)
    }

    /// Start fresh pane state with current preferences and the saved navigation layout.
    ///
    /// # Errors
    /// Returns schema/path validation errors. The caller checks filesystem access.
    pub fn restore(&self, settings: &Settings) -> Result<App, String> {
        let [left, right] = self.locations()?;
        let mut app = App::new(left);
        app.panes[1].path = right;
        settings.apply(&mut app);
        app.active = self.active;
        app.sidebar = app.panes[app.active].sidebar;
        app.dual = self.dual;
        app.maximized = self.maximized;
        app.split_percent = self.split_percent;
        Ok(app)
    }
}

/// Explicit storage, separate from restart preferences and view profiles.
pub struct SessionStore {
    pub path: PathBuf,
}

impl SessionStore {
    /// Atomically replace only a regular destination using an owned synced stage.
    ///
    /// # Errors
    /// Leaves old bytes intact on staging errors; refuses linked/nonregular destinations.
    pub fn save(&self, app: &App) -> Result<(), String> {
        let session = NavigationSession::capture(app);
        session.locations()?;
        let text = toml::to_string_pretty(&session).map_err(|e| e.to_string())?;
        crate::settings::atomic_write(&self.path, &text).map_err(|e| self.error(e))
    }

    /// Load a bounded UTF-8 schema containing raw native path units.
    ///
    /// # Errors
    /// Missing, linked, inaccessible, oversized and invalid sessions fail explicitly.
    pub fn load(&self) -> Result<NavigationSession, String> {
        crate::settings::regular_or_missing(&self.path).map_err(|e| self.error(e))?;
        let file = std::fs::File::open(&self.path).map_err(|e| self.error(e))?;
        let mut text = String::new();
        file.take(1_048_577)
            .read_to_string(&mut text)
            .map_err(|e| self.error(e))?;
        if text.len() > 1_048_576 {
            return Err(self.error("session exceeds 1 MiB"));
        }
        NavigationSession::parse(&text).map_err(|e| self.error(e))
    }

    fn error(&self, error: impl std::fmt::Display) -> String {
        format!("{}: {error}", self.path.display())
    }
}
