//! Version-gated reconciliation preserves configured suffixes until explicit approval.
use super::Settings;
use semver::Version;

/// Only one startup dialog is active at a time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Replacement,
    Additions,
}

/// A startup review owns the exact lists shown to the user.
#[derive(Clone, Debug)]
pub struct Review {
    pub version: String,
    pub configured: Option<Vec<String>>,
    pub defaults: Vec<String>,
    pub additions: Vec<String>,
    pub stage: Stage,
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
        let mut review = Self {
            version: version.into(),
            configured: settings.compound_extensions.clone(),
            defaults: Settings::default().effective_compound_extensions(),
            additions: settings.compound_extensions_extend.clone(),
            stage: Stage::Replacement,
            selected: 0,
            scroll: 0,
        };
        if !review.replacement_differs() {
            review.stage = Stage::Additions;
        }
        (review.stage == Stage::Replacement || !review.removable().is_empty()).then_some(review)
    }

    pub fn replacement_differs(&self) -> bool {
        self.configured.as_ref().is_some_and(|configured| {
            !difference(configured, &self.defaults).is_empty()
                || !difference(&self.defaults, configured).is_empty()
        })
    }

    /// Removing these entries leaves the chosen base's effective suffixes unchanged.
    pub fn removable(&self) -> Vec<String> {
        let base = self.configured.as_ref().unwrap_or(&self.defaults);
        self.additions
            .iter()
            .filter(|suffix| base.contains(suffix))
            .cloned()
            .collect()
    }

    /// Resolve the replacement first, then recalculate whether cleanup is relevant.
    pub fn choose_replacement(&mut self, use_defaults: bool) -> bool {
        if use_defaults {
            self.configured = None;
        }
        self.stage = Stage::Additions;
        self.selected = 0;
        self.scroll = 0;
        !self.removable().is_empty()
    }

    /// Save approved replacement choices and optionally the identified safe cleanup.
    pub fn complete(self, settings: &mut Settings, remove: bool) {
        if remove && self.stage == Stage::Additions {
            let removable = self.removable();
            settings
                .compound_extensions_extend
                .retain(|s| !removable.contains(s));
        }
        settings.compound_extensions = self.configured;
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
