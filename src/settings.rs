//! Optional TOML preferences; no eager writes or navigation persistence.
use crate::{
    app::{App, IgnoreMode},
    input::{Action, Bindings},
    listing::{Grouping, Sort, SortPolicy},
};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Preferred metadata action; direct sidebar and popup actions remain available.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetadataPresentation {
    Sidebar,
    #[default]
    Popup,
}

/// M1 preferences. Later milestones own additional theme/integration settings.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub sidebar: bool,
    pub two_panes: bool,
    pub restore_session: bool,
    pub split_percent: u16,
    pub sidebar_left: bool,
    pub mouse: bool,
    pub sort: Sort,
    pub grouping: Grouping,
    pub reverse: bool,
    pub hidden: bool,
    pub ignore: IgnoreMode,
    pub human_sizes: bool,
    pub fuzzy_timestamps: bool,
    pub owner_display: crate::owner::OwnerDisplay,
    pub permissions: crate::permissions::PermissionDisplay,
    pub columns: crate::view::Columns,
    pub profiles: BTreeMap<String, crate::view::ViewSettings>,
    pub metadata_presentation: MetadataPresentation,
    pub bindings: BTreeMap<String, Action>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sidebar: false,
            two_panes: false,
            restore_session: false,
            split_percent: 50,
            sidebar_left: false,
            mouse: true,
            sort: Sort::Natural,
            grouping: Grouping::First,
            reverse: false,
            hidden: false,
            ignore: IgnoreMode::Dim,
            human_sizes: true,
            fuzzy_timestamps: false,
            owner_display: crate::owner::OwnerDisplay::default(),
            permissions: crate::permissions::PermissionDisplay::default(),
            columns: crate::view::Columns::default(),
            profiles: crate::view::profiles(),
            metadata_presentation: MetadataPresentation::default(),
            bindings: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Explicit profile application updates restart defaults without mutating its source.
    pub fn capture_view(&mut self, view: &crate::view::ViewSettings) {
        self.human_sizes = view.human_sizes;
        self.fuzzy_timestamps = view.fuzzy_timestamps;
        self.owner_display = view.owner_display;
        self.permissions = view.permissions;
        self.columns = view.columns.clone();
        self.sort = view.sort;
        self.grouping = view.grouping;
        self.reverse = view.reverse;
        self.hidden = view.hidden;
        self.ignore = view.ignore;
        if let Some(sidebar) = view.sidebar {
            self.sidebar = sidebar;
        }
    }

    /// Parse strict settings and detect malformed/conflicting keybindings.
    ///
    /// # Errors
    /// Returns the TOML or normalized-key diagnostic without resetting preferences.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut settings: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if !(20..=80).contains(&settings.split_percent) {
            return Err("split_percent must be between 20 and 80".into());
        }
        for name in settings.profiles.keys() {
            validate_profile_name(name)?;
        }
        for (name, view) in crate::view::profiles() {
            settings.profiles.entry(name).or_insert(view);
        }
        Bindings::new(&settings.bindings)?;
        Ok(settings)
    }

    /// Apply startup/view preferences without replacing native locations.
    pub fn apply(&self, app: &mut App) {
        app.sidebar = self.sidebar;
        app.dual = self.two_panes;
        app.split_percent = self.split_percent;
        app.sidebar_left = self.sidebar_left;
        app.metadata_presentation = self.metadata_presentation;
        for pane in &mut app.panes {
            pane.sidebar = self.sidebar;
            pane.sort = SortPolicy {
                sort: self.sort,
                grouping: self.grouping,
                reverse: self.reverse,
            };
            pane.show_hidden = self.hidden;
            pane.ignore = self.ignore;
            pane.human_sizes = self.human_sizes;
            pane.fuzzy_timestamps = self.fuzzy_timestamps;
            pane.owner_display = self.owner_display;
            pane.permissions = self.permissions;
            pane.columns = self.columns.clone();
            pane.resort();
        }
    }

    /// Capture only the explicit change; pane switches never choose restart defaults.
    pub fn capture(&mut self, app: &App, action: Action) {
        let pane = &app.panes[app.active];
        match action {
            Action::HumanSizes => self.human_sizes = pane.human_sizes,
            Action::FuzzyTimestamps => self.fuzzy_timestamps = pane.fuzzy_timestamps,
            Action::OwnerDisplay => self.owner_display = pane.owner_display,
            Action::Permissions => self.permissions = pane.permissions,
            Action::ColumnSize => self.columns.size = pane.columns.size,
            Action::ColumnModified => self.columns.modified = pane.columns.modified,
            Action::ColumnAttributes => self.columns.attributes = pane.columns.attributes,
            Action::ColumnOwner => self.columns.owner = pane.columns.owner,
            Action::ToggleSidebar => self.sidebar = app.sidebar,
            Action::Sort => self.sort = pane.sort.sort,
            Action::Group => self.grouping = pane.sort.grouping,
            Action::Reverse => self.reverse = pane.sort.reverse,
            Action::Hidden => self.hidden = pane.show_hidden,
            Action::Ignore => self.ignore = pane.ignore,
            _ => {}
        }
    }
}

/// Storage path is explicit for tests; persistence happens outside input/render.
pub struct Store {
    pub path: PathBuf,
    pub settings: Settings,
}

impl Store {
    /// Missing config uses defaults without creating any files.
    ///
    /// # Errors
    /// Returns invalid settings, access errors or a nonregular/linked config error.
    pub fn load(path: PathBuf) -> Result<Self, String> {
        regular_or_missing(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let settings = match fs::read_to_string(&path) {
            Ok(text) => Settings::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Settings::default(),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        Ok(Self { path, settings })
    }

    /// Save through an owned same-directory stage, retaining old bytes on failure.
    ///
    /// # Errors
    /// Reports serialization/staging/commit failure instead of silently resetting config.
    pub fn save(&self) -> Result<(), String> {
        self.write()
            .map_err(|e| format!("{}: {e}", self.path.display()))
    }

    fn write(&self) -> io::Result<()> {
        let text = toml::to_string_pretty(&self.settings).map_err(io::Error::other)?;
        atomic_write(&self.path, &text)
    }
}

pub(crate) fn atomic_write(path: &Path, text: &str) -> io::Result<()> {
    regular_or_missing(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("destination has no parent"))?;
    fs::create_dir_all(parent)?;
    let stage = prepare_config(parent, text)?;
    regular_or_missing(path)?;
    stage.persist(path).map_err(|e| e.error)?;
    Ok(())
}

fn prepare_config(parent: &Path, text: &str) -> io::Result<tempfile::NamedTempFile> {
    let mut stage = tempfile::NamedTempFile::new_in(parent)?;
    stage.write_all(text.as_bytes())?;
    stage.as_file().sync_all()?;
    Ok(stage)
}

pub(crate) fn regular_or_missing(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.is_symlink() => Err(io::Error::other(
            "destination must be a regular file, not a link",
        )),
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Platform-correct config location, with XDG overrides on Unix and APPDATA on Windows.
pub fn config_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
    #[cfg(unix)]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(default_unix_config)?;
    Some(base.join("tfm/config.toml"))
}

#[cfg(unix)]
fn default_unix_config() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    #[cfg(target_os = "macos")]
    return Some(home.join("Library/Application Support"));
    #[cfg(not(target_os = "macos"))]
    Some(home.join(".config"))
}

/// Profile names are bounded, printable TOML keys, not paths.
///
/// # Errors
/// Rejects empty, padded, overlong or control/bidi-containing names.
pub fn validate_profile_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty()
        || name.trim() != name
        || name.chars().count() > 64
        || crate::ui::clean(name) != name
    {
        Err("profile name must be 1–64 printable characters without surrounding whitespace".into())
    } else {
        Ok(())
    }
}
