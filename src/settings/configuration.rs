//! Runtime configuration owns storage and nonfatal save policy.
use super::{Settings, Store};
use crate::{input::Controller, notifications::Severity};
use std::path::PathBuf;

/// The application requests configuration operations without managing files or recovery.
pub struct Configuration {
    path: Option<PathBuf>,
    settings: Settings,
    save_requested: bool,
}

impl Configuration {
    /// Load the platform's preferences without hiding invalid or inaccessible files.
    ///
    /// # Errors
    /// Returns the affected path and the read or validation diagnostic.
    pub fn open() -> Result<Self, String> {
        Self::load(super::config_path())
    }

    /// Use an explicit location, or retain defaults when no user directory is available.
    /// Missing files are scheduled for creation when notifications become available.
    ///
    /// # Errors
    /// Existing invalid or inaccessible configuration is a strict startup error.
    pub fn load(path: Option<PathBuf>) -> Result<Self, String> {
        let (settings, save_requested) = match &path {
            Some(path) => {
                let (store, missing) = Store::load_state(path.clone())?;
                (store.settings, missing)
            }
            None => (Settings::default(), true),
        };
        Ok(Self {
            path,
            settings,
            save_requested,
        })
    }

    /// Effective startup preferences; callers only work with values.
    pub fn preferences(&self) -> Settings {
        self.settings.clone()
    }

    /// Persist creation, preference changes and completed compatibility checks together.
    /// Consume each change request once; unrelated input never retries a failed write.
    pub fn synchronize(&mut self, controller: &mut Controller) {
        self.save_requested |= std::mem::take(&mut controller.preferences_changed);
        if !std::mem::take(&mut self.save_requested) {
            return;
        }
        self.settings = controller.preferences.clone();
        let result = match &self.path {
            Some(path) => Store {
                path: path.clone(),
                settings: self.settings.clone(),
            }
            .save(),
            None => Err("no user config directory is available".into()),
        };
        if let Err(error) = result {
            controller.app.notifications.notify(
                format!("Could not save configuration: {error}. Settings remain active but unsaved; check the path and permissions. TFM will try again when configuration changes."),
                controller.app.now,
                Severity::Error,
            );
        }
    }
}

#[cfg(test)]
mod tests;
