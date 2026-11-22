//! Version-gated reconciliation preserves configured suffixes until explicit approval.
use super::Settings;
use semver::Version;

/// A startup review owns the exact lists shown to the user.
#[derive(Clone, Debug)]
pub struct Review {
    pub version: String,
    pub configured: Option<Vec<String>>,
    pub defaults: Vec<String>,
    pub additions: Vec<String>,
    pub use_defaults: bool,
    pub remove_redundant: bool,
    pub selected: usize,
    pub scroll: usize,
}

impl Settings {
    /// Compare SemVer precedence, ignoring build metadata.
    ///
    /// # Errors
    /// Reports malformed recorded or running versions without resetting preferences.
    pub fn compound_check_due(&self, running: &str) -> Result<bool, String> {
        let running = Version::parse(running).map_err(|e| format!("TFM version: {e}"))?;
        self.compound_extensions_checked_version
            .as_deref()
            .map(|text| {
                Version::parse(text)
                    .map(|checked| checked.cmp_precedence(&running).is_lt())
                    .map_err(|e| format!("compound_extensions_checked_version: {e}"))
            })
            .unwrap_or(Ok(true))
    }
}

impl Review {
    /// Return a popup only when suffix membership needs a user decision.
    pub fn new(settings: &Settings, version: &str) -> Option<Self> {
        let review = Self {
            version: version.into(),
            configured: settings.compound_extensions.clone(),
            defaults: Settings::default().effective_compound_extensions(),
            additions: settings.compound_extensions_extend.clone(),
            use_defaults: false,
            remove_redundant: false,
            selected: 0,
            scroll: 0,
        };
        (review.replacement_differs() || !review.default_additions().is_empty()).then_some(review)
    }

    pub fn replacement_differs(&self) -> bool {
        self.configured.as_ref().is_some_and(|configured| {
            !difference(configured, &self.defaults).is_empty()
                || !difference(&self.defaults, configured).is_empty()
        })
    }

    /// Entries shared with shipped defaults may still be needed by an explicit replacement.
    pub fn default_additions(&self) -> Vec<String> {
        self.additions
            .iter()
            .filter(|suffix| self.defaults.contains(suffix))
            .cloned()
            .collect()
    }

    /// Removing these entries leaves the selected base's effective suffixes unchanged.
    pub fn removable(&self) -> Vec<String> {
        let base = if self.use_defaults {
            &self.defaults
        } else {
            self.configured.as_ref().unwrap_or(&self.defaults)
        };
        self.default_additions()
            .into_iter()
            .filter(|suffix| base.contains(suffix))
            .collect()
    }

    /// Dismissal declines both changes, even after editing the tentative choices.
    pub fn complete(self, settings: &mut Settings, apply: bool) {
        if apply && self.remove_redundant {
            let removable = self.removable();
            settings
                .compound_extensions_extend
                .retain(|s| !removable.contains(s));
        }
        if apply && self.use_defaults {
            settings.compound_extensions = None;
        }
        settings.compound_extensions_checked_version = Some(self.version);
    }
}

/// Exact, ordered set difference for both the attention check and displayed evidence.
pub fn difference(left: &[String], right: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for suffix in left {
        if !right.contains(suffix) && !result.contains(suffix) {
            result.push(suffix.clone());
        }
    }
    result
}
