use crate::{
    fs::{Entry, Identity, Snapshot},
    listing::{Sort, SortPolicy},
};
use std::{collections::HashMap, path::PathBuf};

/// Hidden, ignored and fuzzy-filter exclusions remain independent.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IgnoreMode {
    Off,
    #[default]
    Dim,
    Hide,
}

#[derive(Clone, Debug)]
pub(super) struct Visit {
    pub path: PathBuf,
    pub cursor: Option<PathBuf>,
    pub scroll: usize,
}

/// Visible and excluded marks use mutually exclusive exclusion reasons.
#[derive(Clone, Copy, Debug, Default)]
pub struct SelectionCounts {
    pub visible: usize,
    pub hidden: usize,
    pub ignored: usize,
    pub filtered: usize,
}

impl SelectionCounts {
    /// Count unique excluded marks, without double counting overlapping rules.
    pub fn excluded(self) -> usize {
        self.hidden + self.ignored + self.filtered
    }
}

/// One independent session-only pane.
pub struct Pane {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub visible: Vec<usize>,
    pub cursor: Option<PathBuf>,
    pub marks: HashMap<PathBuf, Identity>,
    pub scroll: usize,
    pub filter: String,
    pub sort: SortPolicy,
    pub show_hidden: bool,
    pub ignore: IgnoreMode,
    pub human_sizes: bool,
    pub fuzzy_timestamps: bool,
    pub permissions: crate::permissions::PermissionDisplay,
    pub columns: crate::view::Columns,
    pub measurements: crate::ui::Measurements,
    pub generation: u64,
    pub pending: Option<PathBuf>,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    listing_order: HashMap<PathBuf, usize>,
    pub(super) back: Vec<Visit>,
    pub(super) forward: Vec<Visit>,
    pub(super) restoring: Option<Visit>,
    pub(super) history_forward: bool,
}

impl Pane {
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            path,
            entries: Vec::new(),
            visible: Vec::new(),
            cursor: None,
            marks: HashMap::new(),
            scroll: 0,
            filter: String::new(),
            sort: SortPolicy::default(),
            show_hidden: false,
            ignore: IgnoreMode::Dim,
            human_sizes: true,
            fuzzy_timestamps: false,
            permissions: crate::permissions::PermissionDisplay::default(),
            columns: crate::view::Columns::default(),
            measurements: crate::ui::Measurements::default(),
            generation: 1,
            pending: None,
            error: None,
            warnings: Vec::new(),
            listing_order: HashMap::new(),
            back: Vec::new(),
            forward: Vec::new(),
            restoring: None,
            history_forward: false,
        }
    }

    pub(super) fn install(&mut self, snapshot: Snapshot) {
        let navigating = self.path != snapshot.path;
        if navigating {
            if self.restoring.is_some() {
                let previous = self.visit();
                if self.history_forward {
                    self.forward.pop();
                    self.back.push(previous);
                } else {
                    self.back.pop();
                    self.forward.push(previous);
                }
            } else {
                self.back.push(self.visit());
                self.forward.clear();
            }
            self.path = snapshot.path;
            self.marks.clear();
            self.cursor = None;
            self.scroll = 0;
        }
        self.listing_order = snapshot
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.path.clone(), index))
            .collect();
        self.entries = snapshot.entries;
        self.warnings = snapshot.errors;
        self.marks = self
            .entries
            .iter()
            .filter_map(|entry| {
                let previous = self.marks.get(&entry.path)?;
                let fresh = entry.identity.as_ref()?;
                same_object(previous, fresh).then(|| (entry.path.clone(), previous.clone()))
            })
            .collect();
        self.resort();
        if let Some(visit) = self.restoring.take() {
            self.cursor = visit.cursor;
            self.scroll = visit.scroll;
            self.refilter();
        }
    }

    /// Apply sort/grouping while preserving cursor and raw marked paths.
    pub fn resort(&mut self) {
        if self.sort.sort == Sort::Unsorted {
            self.entries
                .sort_by_key(|entry| self.listing_order.get(&entry.path).copied());
        }
        self.sort.apply(&mut self.entries);
        self.refilter();
    }

    /// Recompute the working set only when policy or input changes, never during draw.
    pub fn refilter(&mut self) {
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| self.in_scope(e).then_some(i))
            .collect();
        self.measurements = crate::ui::Measurements::collect(
            self.visible.iter().map(|i| &self.entries[*i]),
            self.human_sizes,
        );
        if !self
            .visible
            .iter()
            .any(|i| Some(&self.entries[*i].path) == self.cursor.as_ref())
        {
            self.cursor = self.visible.first().map(|i| self.entries[*i].path.clone());
            self.scroll = 0;
        }
    }

    /// Live subsequence filtering stays in process; empty results have no fallback.
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.refilter();
    }

    fn in_scope(&self, entry: &Entry) -> bool {
        (self.show_hidden || !entry.hidden)
            && (self.ignore != IgnoreMode::Hide || entry.ignored != Some(true))
            && fuzzy(&entry.name.to_string_lossy(), &self.filter)
    }

    /// Current visible entry, never a stale row index.
    pub fn current(&self) -> Option<&Entry> {
        self.visible
            .iter()
            .map(|i| &self.entries[*i])
            .find(|e| Some(&e.path) == self.cursor.as_ref())
    }

    /// Move within the cached working set, retaining a bounded scroll offset.
    pub fn move_cursor(&mut self, delta: isize, height: usize) {
        let index = self.cursor_index().unwrap_or(0);
        let next = index
            .saturating_add_signed(delta)
            .min(self.visible.len().saturating_sub(1));
        self.cursor = self
            .visible
            .get(next)
            .map(|i| self.entries[*i].path.clone());
        if next < self.scroll {
            self.scroll = next;
        }
        if next >= self.scroll.saturating_add(height.max(1)) {
            self.scroll = next + 1 - height.max(1);
        }
    }

    /// Visible row index for rendering and paging.
    pub fn cursor_index(&self) -> Option<usize> {
        self.visible
            .iter()
            .position(|i| Some(&self.entries[*i].path) == self.cursor.as_ref())
    }

    /// Shared viewport calculation for drawing and mouse row hit testing.
    pub fn viewport_start(&self, height: usize) -> usize {
        let index = self.cursor_index().unwrap_or(0);
        self.scroll
            .min(index)
            .max(index.saturating_add(1).saturating_sub(height.max(1)))
    }

    /// Toggle the cursor's stable native mark.
    pub fn toggle_mark(&mut self) {
        if let Some((path, identity)) = self
            .current()
            .map(|entry| (entry.path.clone(), entry.identity.clone()))
            && self.marks.remove(&path).is_none()
            && let Some(identity) = identity
        {
            self.marks.insert(path, identity);
        }
    }

    /// Mark only the current visible working set.
    pub fn select_all(&mut self) {
        for i in &self.visible {
            let entry = &self.entries[*i];
            if let Some(identity) = &entry.identity {
                self.marks.insert(entry.path.clone(), identity.clone());
            }
        }
    }

    /// Clear visible marks; hidden and filtered marks remain in state.
    pub fn clear_visible(&mut self) {
        for i in &self.visible {
            self.marks.remove(&self.entries[*i].path);
        }
    }

    /// Read-only operand inspection; no mutation dispatcher exists in M1.
    pub fn eligible(&self) -> Vec<&Entry> {
        let marks: Vec<_> = self
            .visible
            .iter()
            .map(|i| &self.entries[*i])
            .filter(|e| self.marks.contains_key(&e.path))
            .collect();
        if marks.is_empty() {
            self.current().into_iter().collect()
        } else {
            marks
        }
    }

    /// Count exclusion reasons once, with hidden then ignore then filter precedence.
    pub fn selection_counts(&self) -> SelectionCounts {
        let mut counts = SelectionCounts::default();
        for entry in self
            .entries
            .iter()
            .filter(|e| self.marks.contains_key(&e.path))
        {
            if !self.show_hidden && entry.hidden {
                counts.hidden += 1;
            } else if self.ignore == IgnoreMode::Hide && entry.ignored == Some(true) {
                counts.ignored += 1;
            } else if !fuzzy(&entry.name.to_string_lossy(), &self.filter) {
                counts.filtered += 1;
            } else {
                counts.visible += 1;
            }
        }
        counts
    }

    pub(super) fn visit(&self) -> Visit {
        Visit {
            path: self.path.clone(),
            cursor: self.cursor.clone(),
            scroll: self.scroll,
        }
    }
}

fn same_object(a: &Identity, b: &Identity) -> bool {
    match (a.object, b.object) {
        (Some(left), Some(right)) => left == right && a.created == b.created,
        _ => a == b,
    }
}

fn fuzzy(name: &str, query: &str) -> bool {
    let lowered = name.to_lowercase();
    let mut chars = lowered.chars();
    query
        .to_lowercase()
        .chars()
        .all(|needle| chars.any(|c| c == needle))
}
