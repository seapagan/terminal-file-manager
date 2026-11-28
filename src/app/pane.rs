use crate::{
    fs::{Entry, EntryKind, Identity, Snapshot},
    listing::{Sort, SortPolicy},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

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
    pub directory_identity: Option<Identity>,
    pub cursor_identity: Option<Identity>,
    pub entered: Option<(PathBuf, EntryKind, Identity)>,
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
    pub(super) remote: Option<super::remote::Location>,
    pub directory: Option<Entry>,
    pub(crate) entry_identity: Option<Identity>,
    pub path: Option<PathBuf>,
    pub entries: Vec<Entry>,
    pub visible: Vec<usize>,
    pub cursor: Option<PathBuf>,
    pub marks: HashMap<PathBuf, Identity>,
    pub scroll: usize,
    pub listing_installs: u64,
    pub filter: String,
    pub filter_editor: Option<crate::editor::Editor>,
    pub sort: SortPolicy,
    pub show_hidden: bool,
    pub ignore: IgnoreMode,
    pub human_sizes: bool,
    pub fuzzy_timestamps: bool,
    pub owner_display: crate::owner::OwnerDisplay,
    pub permissions: crate::permissions::PermissionDisplay,
    pub columns: crate::view::Columns,
    pub sidebar: bool,
    pub profile: Option<String>,
    pub profile_modified: bool,
    pub measurements: crate::ui::Measurements,
    pub generation: u64,
    pub pending: Option<PathBuf>,
    pub(crate) listing: bool,
    pub(crate) refresh_pending: bool,
    pub(crate) context: u64,
    pub(super) selection_revision: u64,
    pub(super) creation: Option<super::creation::CreationFocus>,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub(super) range: Option<super::selection::RangeSelection>,
    pub(super) listing_order: HashMap<PathBuf, usize>,
    pub(super) back: Vec<Visit>,
    pub(super) forward: Vec<Visit>,
    pub(super) restoring: Option<Visit>,
    pub(super) parent_restoring: Option<super::navigation::ParentRestore>,
    pub(super) history_forward: bool,
}

impl Pane {
    /// Native directory path, absent in a remote pane.
    pub fn native_path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    /// The native directory for code operating exclusively on local panes.
    pub fn local_path(&self) -> &PathBuf {
        self.native_path().expect("native pane")
    }
    pub(super) fn accepts_refresh(&self, snapshot: &Snapshot) -> bool {
        let Some(previous) = &self.directory else {
            return self.context == 0;
        };
        if Some(&snapshot.path) != self.native_path() {
            return false;
        }
        snapshot.directory.as_ref().is_some_and(|fresh| {
            previous.path == fresh.path
                && previous
                    .identity
                    .as_ref()
                    .zip(fresh.identity.as_ref())
                    .is_some_and(|(a, b)| same_object(a, b))
        })
    }

    /// Profile identity remains separate from navigation and saved profile bytes.
    pub fn profile_label(&self) -> String {
        match (&self.profile, self.profile_modified) {
            (Some(name), true) => format!("Modified: {name}"),
            (Some(name), false) => name.clone(),
            _ => "Custom".into(),
        }
    }

    pub(super) fn new(path: Option<PathBuf>) -> Self {
        Self {
            remote: None,
            cursor: path
                .as_deref()
                .and_then(Path::parent)
                .filter(|p| !p.as_os_str().is_empty())
                .map(PathBuf::from),
            entry_identity: None,
            directory: None,
            path,
            entries: Vec::new(),
            visible: Vec::new(),
            marks: HashMap::new(),
            scroll: 0,
            listing_installs: 0,
            filter: String::new(),
            filter_editor: None,
            sort: SortPolicy::default(),
            show_hidden: false,
            ignore: IgnoreMode::Dim,
            human_sizes: true,
            fuzzy_timestamps: false,
            owner_display: crate::owner::OwnerDisplay::default(),
            permissions: crate::permissions::PermissionDisplay::default(),
            columns: crate::view::Columns::default(),
            sidebar: false,
            profile: None,
            profile_modified: false,
            measurements: crate::ui::Measurements::default(),
            generation: 1,
            pending: None,
            listing: true,
            refresh_pending: false,
            context: 0,
            selection_revision: 0,
            creation: None,
            error: None,
            warnings: Vec::new(),
            range: None,
            listing_order: HashMap::new(),
            back: Vec::new(),
            forward: Vec::new(),
            restoring: None,
            parent_restoring: None,
            history_forward: false,
        }
    }

    pub(super) fn install(&mut self, snapshot: Snapshot) {
        let previous_index = self.cursor_index().unwrap_or(0);
        let continuous = self.accepts_refresh(&snapshot);
        #[cfg(windows)]
        let snapshot = self.preserve_owners(snapshot, continuous);
        let creation = self.take_creation(continuous);
        let range = self.range.take().filter(|_| continuous);
        if !continuous {
            self.marks.clear();
        }
        self.reset_range();
        let navigating = self.native_path() != Some(&snapshot.path);
        if navigating {
            let previous = self.visit(&snapshot.path);
            let direction = self.restoring.as_ref().map(|_| self.history_forward);
            super::navigation::record_visit(&mut self.back, &mut self.forward, previous, direction);
            self.path = Some(snapshot.path);
            self.marks.clear();
            self.cursor = self.local_path().parent().map(PathBuf::from);
            self.scroll = 0;
        }
        self.directory = snapshot.directory.map(|entry| *entry);
        self.listing_order = snapshot
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.local_path().clone(), index))
            .collect();
        self.entries = snapshot.entries;
        self.warnings = snapshot.errors;
        self.reconcile_marks();
        self.sort.apply(&mut self.entries);
        self.refilter_at(if navigating { 0 } else { previous_index });
        self.restore_cursor();
        if !navigating {
            self.range = range;
            self.reconcile_range();
        }
        self.restore_creation(creation);
    }

    fn reconcile_marks(&mut self) {
        self.marks = self
            .entries
            .iter()
            .filter_map(|entry| {
                let previous = self.marks.get(entry.local_path())?;
                let fresh = entry.identity.as_ref()?;
                same_object(previous, fresh).then(|| (entry.local_path().clone(), previous.clone()))
            })
            .collect();
    }

    #[cfg(windows)]
    fn preserve_owners(&self, mut snapshot: Snapshot, continuous: bool) -> Snapshot {
        if !continuous {
            return snapshot;
        }
        let previous: HashMap<_, _> = self
            .entries
            .iter()
            .map(|entry| (&entry.path, entry))
            .collect();
        for fresh in &mut snapshot.entries {
            if let Some(old) = previous.get(&fresh.path)
                && old
                    .identity
                    .as_ref()
                    .zip(fresh.identity.as_ref())
                    .is_some_and(|(a, b)| a.object.is_some() && same_object(a, b))
            {
                fresh.windows_owner = old.windows_owner.clone();
            }
        }
        snapshot
    }

    /// Apply sort/grouping while preserving cursor and raw marked paths.
    pub fn resort(&mut self) {
        let previous_index = self.cursor_index().unwrap_or(0);
        self.reset_range();
        if self.sort.sort == Sort::Unsorted {
            self.entries.sort_by_key(|entry| {
                entry
                    .remote
                    .as_ref()
                    .map(|remote| remote.order)
                    .or_else(|| self.listing_order.get(entry.local_path()).copied())
            });
        }
        self.sort.apply(&mut self.entries);
        self.refilter_at(previous_index);
    }

    /// Recompute the working set only when policy or input changes, never during draw.
    pub fn refilter(&mut self) {
        let previous_index = self.cursor_index().unwrap_or(0);
        self.refilter_at(previous_index);
    }

    pub(super) fn refilter_at(&mut self, previous_index: usize) {
        self.selection_revision = self.selection_revision.wrapping_add(1);
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| self.in_scope(e).then_some(i))
            .collect();
        self.measurements = crate::ui::Measurements::collect(
            self.visible.iter().map(|i| &self.entries[*i]),
            self.human_sizes,
            self.permissions,
            self.owner_display,
        );
        if !self.parent_focused() && self.cursor_index().is_none() {
            let index = previous_index.min(self.visible.len().saturating_sub(1));
            let entry = self.visible.get(index).copied();
            self.set_entry_focus(entry);
            self.scroll = self.scroll.min(index);
        }
    }

    /// Live subsequence filtering stays in process; empty results have no fallback.
    pub fn set_filter(&mut self, filter: String) {
        self.reset_range();
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
            .find(|e| self.entry_focused(e))
    }

    /// Move within the cached working set, retaining a bounded scroll offset.
    pub fn move_cursor(&mut self, delta: isize, height: usize) {
        self.shift_cursor(delta, height);
        self.anchor_range();
    }

    pub(super) fn shift_cursor(&mut self, delta: isize, height: usize) {
        self.scroll = self.viewport_start(height);
        let index = self.row_index().unwrap_or(0);
        let next = index
            .saturating_add_signed(delta)
            .min(self.row_count().saturating_sub(1));
        self.focus_row(next);
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
            .position(|i| self.entries.get(*i).is_some_and(|e| self.entry_focused(e)))
    }

    /// Shared viewport calculation for drawing and mouse row hit testing.
    pub fn viewport_start(&self, height: usize) -> usize {
        let index = self.row_index().unwrap_or(0);
        self.scroll
            .min(index)
            .max(index.saturating_add(1).saturating_sub(height.max(1)))
    }

    /// Toggle the cursor's stable native mark.
    pub fn toggle_mark(&mut self) {
        self.selection_revision = self.selection_revision.wrapping_add(1);
        self.reset_range();
        if let Some((path, identity)) = self
            .current()
            .and_then(|entry| Some((entry.native_path()?.clone(), entry.identity.clone())))
            && self.marks.remove(&path).is_none()
            && let Some(identity) = identity
        {
            self.marks.insert(path, identity);
        }
    }

    /// Mark only the current visible working set.
    pub fn select_all(&mut self) {
        self.selection_revision = self.selection_revision.wrapping_add(1);
        self.reset_range();
        for i in &self.visible {
            let entry = &self.entries[*i];
            if let Some(identity) = &entry.identity {
                self.marks
                    .insert(entry.local_path().clone(), identity.clone());
            }
        }
    }

    /// Clear visible marks; hidden and filtered marks remain in state.
    pub fn clear_visible(&mut self) {
        self.selection_revision = self.selection_revision.wrapping_add(1);
        self.reset_range();
        for i in &self.visible {
            if let Some(path) = self.entries[*i].native_path() {
                self.marks.remove(path);
            }
        }
    }

    /// Read-only operand inspection; no mutation dispatcher exists in M1.
    pub fn eligible(&self) -> Vec<&Entry> {
        if self.is_remote() {
            return Vec::new();
        }
        let marks: Vec<_> = self
            .visible
            .iter()
            .map(|i| &self.entries[*i])
            .filter(|e| {
                e.native_path()
                    .is_some_and(|path| self.marks.contains_key(path))
            })
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
        for entry in self.entries.iter().filter(|e| {
            e.native_path()
                .is_some_and(|path| self.marks.contains_key(path))
        }) {
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

    fn visit(&self, destination: &Path) -> Visit {
        Visit {
            path: self.local_path().clone(),
            cursor: self.cursor.clone(),
            scroll: self.scroll,
            directory_identity: self.directory.as_ref().and_then(|e| e.identity.clone()),
            cursor_identity: self.current().and_then(|e| e.identity.clone()),
            entered: self
                .entries
                .iter()
                .find(|entry| entry.local_path() == destination)
                .and_then(|entry| {
                    Some((
                        entry.local_path().clone(),
                        entry.kind,
                        entry.identity.clone()?,
                    ))
                }),
        }
    }

    pub(super) fn parent_visit(&self, parent: PathBuf) -> Visit {
        let saved = self
            .back
            .iter()
            .rev()
            .chain(self.forward.iter().rev())
            .find(|visit| visit.path == parent);
        let identity = saved
            .and_then(|visit| visit.entered.as_ref())
            .filter(|(path, kind, _)| {
                Some(path) == self.native_path()
                    && matches!(kind, EntryKind::Link | EntryKind::Junction)
            })
            .map(|(_, _, identity)| identity.clone())
            .or_else(|| self.directory.as_ref().and(self.entry_identity.clone()));
        Visit {
            path: parent,
            cursor: Some(self.local_path().clone()),
            scroll: saved.map_or(0, |visit| visit.scroll),
            directory_identity: saved.and_then(|visit| visit.directory_identity.clone()),
            cursor_identity: identity,
            entered: None,
        }
    }

    pub(super) fn restore_parent_visit(&mut self, visit: &Visit) -> bool {
        let Some(cursor) = self.visit_cursor(visit) else {
            return false;
        };
        self.cursor = Some(cursor);
        if self.matches_visit(visit) {
            self.scroll = visit.scroll.min(self.row_index().unwrap_or(0));
        }
        true
    }

    fn visit_cursor(&self, visit: &Visit) -> Option<PathBuf> {
        self.visible
            .iter()
            .map(|i| &self.entries[*i])
            .find(|entry| {
                entry.native_path() == visit.cursor.as_ref()
                    && entry
                        .identity
                        .as_ref()
                        .zip(visit.cursor_identity.as_ref())
                        .is_some_and(|(fresh, old)| same_object(fresh, old))
            })
            .map(|entry| entry.local_path().clone())
    }

    fn matches_visit(&self, visit: &Visit) -> bool {
        self.directory
            .as_ref()
            .and_then(|entry| entry.identity.as_ref())
            .zip(visit.directory_identity.as_ref())
            .is_some_and(|(fresh, old)| same_object(fresh, old))
    }

    fn restore_cursor(&mut self) {
        if let Some(visit) = self.restoring.take()
            && self.matches_visit(&visit)
        {
            self.cursor = if visit.cursor.as_deref() == self.local_path().parent() {
                visit.cursor.clone()
            } else {
                self.visit_cursor(&visit)
            };
            self.scroll = visit.scroll;
            self.refilter();
        }
        self.restore_parent();
    }
}

pub(super) fn same_object(a: &Identity, b: &Identity) -> bool {
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
