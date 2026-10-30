//! Pattern marking is bounded by the native identities visible when opened.
use super::{Pane, pane::same_object};
use crate::fs::Identity;
use std::{cell::RefCell, collections::HashMap, path::PathBuf};

#[derive(Eq, PartialEq)]
struct PreviewKey {
    revision: u64,
    mode: PatternMode,
    case_sensitive: bool,
    generation: u64,
    context: u64,
    unavailable: bool,
    path: PathBuf,
    directory: Option<Identity>,
}

struct CachedPreview {
    key: PreviewKey,
    result: Result<PatternPreview, String>,
}

/// Filename matching syntax, independent of the listing's fuzzy filter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatternMode {
    /// Globset whole-filename wildcards.
    Wildcard,
    /// Rust regex substring matching; anchors are explicit.
    Regex,
}

/// An explicit change to marks, never an operation on the cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatternOperation {
    /// Add matching visible identities.
    Mark,
    /// Remove matching visible marks.
    Unmark,
    /// Replace only marks in the captured visible working set.
    Replace,
}

/// Exact form target shared by model focus, keyboard and mouse input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatternControl {
    Expression,
    Mode(PatternMode),
    Case,
    Apply(PatternOperation),
    Cancel,
}

/// Live counts from the same validated scope used by Apply.
#[derive(Clone, Copy, Debug)]
pub struct PatternPreview {
    pub matches: usize,
    pub visible: usize,
    pub marked: usize,
}

/// Session-only form with a frozen native working set and cached compiled matcher.
pub struct PatternSelection {
    pub pane: usize,
    /// Deliberate editor or control focus.
    pub focus: PatternControl,
    pub feedback: Option<String>,
    rejected_edit: bool,
    pub scroll: usize,
    pub mode: PatternMode,
    pub case_sensitive: bool,
    expression: String,
    matcher: Result<regex::bytes::Regex, String>,
    path: PathBuf,
    context: u64,
    directory: Option<Identity>,
    scope: HashMap<PathBuf, Identity>,
    preview_cache: RefCell<Option<CachedPreview>>,
    #[cfg(test)]
    preview_scans: std::cell::Cell<usize>,
}

impl PatternSelection {
    /// Capture only selectable visible names, including directories.
    pub fn new(pane: &Pane, id: usize) -> Self {
        Self {
            pane: id,
            focus: PatternControl::Expression,
            feedback: None,
            rejected_edit: false,
            scroll: 0,
            mode: PatternMode::Wildcard,
            case_sensitive: true,
            expression: String::new(),
            preview_cache: RefCell::new(None),
            matcher: Err("Enter a non-empty pattern".into()),
            path: pane.path.clone(),
            context: pane.context,
            directory: pane.directory.as_ref().and_then(|e| e.identity.clone()),
            scope: selectable(pane)
                .map(|e| (e.path.clone(), e.identity.clone().unwrap()))
                .collect(),
            #[cfg(test)]
            preview_scans: std::cell::Cell::new(0),
        }
    }

    /// Original user expression, never a filename authority.
    pub fn expression(&self) -> &str {
        &self.expression
    }

    /// Compile only when edited, with bounded regex allocation and parser limits.
    pub fn set_expression(&mut self, text: String) {
        self.preview_cache.get_mut().take();
        self.feedback = None;
        if text.len() > 4096 {
            self.reject_edit();
            return;
        }
        self.rejected_edit = false;
        self.expression = text;
        self.matcher = self.compile();
    }

    /// Append a complete edit or reject it without retaining any prefix.
    pub fn append(&mut self, text: &str) {
        if self.rejected_edit {
            return;
        }
        let mut expression = self.expression.clone();
        for c in text.chars().filter(|c| !c.is_control()) {
            if expression.len() + c.len_utf8() > 4096 {
                self.reject_edit();
                return;
            }
            expression.push(c);
        }
        self.set_expression(expression);
    }

    fn reject_edit(&mut self) {
        self.rejected_edit = true;
        self.preview_cache.get_mut().take();
        self.matcher = self.compile();
    }

    /// Change syntax without changing the approved working set.
    pub fn set_mode(&mut self, mode: PatternMode) {
        self.preview_cache.get_mut().take();
        self.feedback = None;
        self.mode = mode;
        self.matcher = self.compile();
    }

    /// Matching starts case-sensitive on every open.
    pub fn toggle_case(&mut self) {
        self.preview_cache.get_mut().take();
        self.feedback = None;
        self.case_sensitive = !self.case_sensitive;
        self.matcher = self.compile();
    }

    fn compile(&self) -> Result<regex::bytes::Regex, String> {
        if self.rejected_edit {
            return Err("Edit exceeds 4096 bytes; Backspace to correct or Esc to cancel".into());
        }
        if self.expression.is_empty() {
            return Err("Enter a non-empty pattern".into());
        }
        if self.expression.len() > 4096 {
            return Err("Pattern exceeds 4096 bytes".into());
        }
        let pattern = match self.mode {
            PatternMode::Wildcard => globset::GlobBuilder::new(&self.expression)
                .backslash_escape(true)
                .case_insensitive(!self.case_sensitive)
                .build()
                .map_err(|e| e.to_string())?
                .regex()
                .to_owned(),
            PatternMode::Regex => self.expression.clone(),
        };
        regex::bytes::RegexBuilder::new(&pattern)
            .case_insensitive(!self.case_sensitive)
            .size_limit(2 * 1024 * 1024)
            .build()
            .map_err(|e| e.to_string())
    }

    fn validate(&self, pane: &Pane) -> Result<(), String> {
        let same_directory = self
            .directory
            .as_ref()
            .zip(pane.directory.as_ref().and_then(|e| e.identity.as_ref()))
            .is_some_and(|(a, b)| same_object(a, b));
        if pane.pending.is_some() || pane.listing || pane.error.is_some() {
            return Err("Listing unavailable or refreshing; wait before applying".into());
        }
        if pane.path != self.path || pane.context != self.context || !same_directory {
            return Err("Directory changed; cancel and reopen pattern selection".into());
        }
        let mut count = 0;
        for entry in selectable(pane) {
            count += 1;
            if !self
                .scope
                .get(&entry.path)
                .zip(entry.identity.as_ref())
                .is_some_and(|(a, b)| same_object(a, b))
            {
                return Err("Visible entries changed; cancel and reopen pattern selection".into());
            }
        }
        if count != self.scope.len() {
            return Err("Visible entries changed; cancel and reopen pattern selection".into());
        }
        Ok(())
    }

    /// Compute counts without lossy conversion, I/O or changes to marks.
    pub fn preview(&self, pane: &Pane) -> Result<PatternPreview, String> {
        #[cfg(test)]
        self.preview_scans.set(self.preview_scans.get() + 1);
        self.validate(pane)?;
        let matcher = self.matcher.as_ref().map_err(Clone::clone)?;
        Ok(PatternPreview {
            matches: selectable(pane)
                .filter(|e| matcher.is_match(e.name.as_encoded_bytes()))
                .count(),
            visible: self.scope.len(),
            marked: self
                .scope
                .keys()
                .filter(|path| pane.marks.contains_key(*path))
                .count(),
        })
    }

    /// Rendering reuses counts until a model mutation invalidates them.
    pub(crate) fn cached_preview(&self, pane: &Pane) -> Result<PatternPreview, String> {
        let key = PreviewKey {
            revision: pane.selection_revision,
            mode: self.mode,
            case_sensitive: self.case_sensitive,
            generation: pane.generation,
            context: pane.context,
            unavailable: pane.pending.is_some() || pane.listing || pane.error.is_some(),
            path: pane.path.clone(),
            directory: pane.directory.as_ref().and_then(|e| e.identity.clone()),
        };
        let mut cache = self.preview_cache.borrow_mut();
        if cache.as_ref().is_none_or(|cached| cached.key != key) {
            *cache = Some(CachedPreview {
                key,
                result: self.preview(pane),
            });
        }
        cache.as_ref().unwrap().result.clone()
    }

    /// Revalidate immediately, then apply only this explicit visible-mark change.
    ///
    /// # Errors
    /// Invalid expressions and changed/unavailable listings leave marks untouched.
    pub fn apply(
        &self,
        pane: &mut Pane,
        operation: PatternOperation,
    ) -> Result<PatternPreview, String> {
        // Validate the entire live scope before changing even the first mark.
        self.validate(pane)?;
        let matcher = self.matcher.as_ref().map_err(Clone::clone)?;
        let mut preview = PatternPreview {
            matches: 0,
            visible: self.scope.len(),
            marked: self
                .scope
                .keys()
                .filter(|path| pane.marks.contains_key(*path))
                .count(),
        };
        pane.reset_range();
        pane.selection_revision = pane.selection_revision.wrapping_add(1);
        for i in &pane.visible {
            let entry = &pane.entries[*i];
            if self.scope.contains_key(&entry.path) {
                let matched = matcher.is_match(entry.name.as_encoded_bytes());
                preview.matches += usize::from(matched);
                self.apply_entry(entry, &mut pane.marks, matched, operation);
            }
        }
        Ok(preview)
    }

    fn apply_entry(
        &self,
        entry: &crate::fs::Entry,
        marks: &mut HashMap<PathBuf, Identity>,
        matched: bool,
        operation: PatternOperation,
    ) {
        let Some(identity) = self.scope.get(&entry.path) else {
            return;
        };
        match operation {
            PatternOperation::Mark | PatternOperation::Replace if matched => {
                marks
                    .entry(entry.path.clone())
                    .or_insert_with(|| identity.clone());
            }
            PatternOperation::Replace => {
                marks.remove(&entry.path);
            }
            PatternOperation::Unmark if matched => {
                marks.remove(&entry.path);
            }
            _ => {}
        }
    }
}

fn selectable(pane: &Pane) -> impl Iterator<Item = &crate::fs::Entry> {
    pane.visible
        .iter()
        .map(|i| &pane.entries[*i])
        .filter(|e| e.identity.is_some())
}

#[cfg(test)]
mod tests;
