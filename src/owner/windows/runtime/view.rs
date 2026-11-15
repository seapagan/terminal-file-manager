//! Pick current metadata and viewport entries, and install cached state only.
use super::{Key, Ownership};
use crate::{
    app::{App, Pane},
    owner::OwnerDisplay,
    ui::Geometry,
};

pub(super) struct Target {
    pub key: Key,
    pub pane: usize,
    index: Option<usize>,
    detail: bool,
    pub names: bool,
}

impl Target {
    pub fn known<'a>(&self, app: &'a App) -> &'a Ownership {
        if self.detail {
            &app.details[self.pane]
                .entry
                .as_ref()
                .expect("current metadata")
                .windows_owner
        } else {
            &app.panes[self.pane].entries[self.index.expect("viewport entry")].windows_owner
        }
    }
}

pub(super) fn targets(app: &App, geometry: &Geometry) -> Vec<Target> {
    let mut targets = Vec::new();
    if let Some(target) = metadata(app, geometry) {
        targets.push(target);
    }
    for id in [app.active, 1 - app.active] {
        let pane = &app.panes[id];
        let height = geometry.listing_rows[id];
        if geometry.panes[id].is_none() || pane.listing || height == 0 {
            continue;
        }
        let (start, _) = viewport(pane, height);
        let parent = usize::from(pane.has_parent());
        for row in start..start.saturating_add(height).min(pane.row_count()) {
            let Some(index) = row
                .checked_sub(parent)
                .and_then(|row| pane.visible.get(row))
                .copied()
            else {
                continue;
            };
            if let Some(key) = Key::new(&pane.entries[index]) {
                targets.push(Target {
                    key,
                    pane: id,
                    index: Some(index),
                    detail: false,
                    names: pane.owner_display == OwnerDisplay::Names,
                });
            }
        }
    }
    targets
}

fn metadata(app: &App, geometry: &Geometry) -> Option<Target> {
    if !app.popup && geometry.sidebar.is_none() {
        return None;
    }
    let pane = &app.panes[app.active];
    if pane.listing {
        return None;
    }
    let detail = &app.details[app.active];
    let entry = detail.entry.as_ref()?;
    if detail.path.as_ref() != Some(&entry.path) || pane.cursor.as_ref() != Some(&entry.path) {
        return None;
    }
    let key = Key::new(entry)?;
    let (_, index) = viewport(pane, geometry.listing_rows[app.active]);
    let index = index.filter(|index| key.matches(&pane.entries[*index]));
    let names = requested_names(app, geometry);
    Some(Target {
        key,
        pane: app.active,
        index,
        detail: true,
        names,
    })
}

fn viewport(pane: &Pane, height: usize) -> (usize, Option<usize>) {
    if pane.parent_focused() || pane.cursor.is_none() {
        return (0, None);
    }
    // Normal input already keeps the cursor inside scroll..scroll+height.
    // Check only that bounded range instead of scanning every directory entry.
    if let Some(index) = cursor_in_rows(pane, pane.scroll, height) {
        return (pane.scroll, Some(index));
    }
    // Resize, sorting and restored focus can put the cursor outside that range.
    // Retain the canonical viewport contract for those transitions.
    let start = pane.viewport_start(height);
    (start, cursor_in_rows(pane, start, height))
}

fn cursor_in_rows(pane: &Pane, start: usize, height: usize) -> Option<usize> {
    let parent = usize::from(pane.has_parent());
    let first = start.saturating_sub(parent);
    let end = start
        .saturating_add(height.max(1))
        .saturating_sub(parent)
        .min(pane.visible.len());
    pane.visible
        .get(first..end)?
        .iter()
        .copied()
        .find(|index| Some(&pane.entries[*index].path) == pane.cursor.as_ref())
}

fn requested_names(app: &App, geometry: &Geometry) -> bool {
    (app.popup && app.metadata_format(true).owner_display == OwnerDisplay::Names)
        || (geometry.sidebar.is_some()
            && app.metadata_format(false).owner_display == OwnerDisplay::Names)
}

pub(super) fn apply(app: &mut App, target: &Target, owner: Ownership) -> bool {
    let pane = &mut app.panes[target.pane];
    let mut changed = false;
    if let Some(index) = target.index {
        let entry = &mut pane.entries[index];
        if target.key.matches(entry) && entry.windows_owner != owner {
            let before = entry.owner_display(pane.owner_display);
            entry.windows_owner = owner.clone();
            pane.measurements
                .update_owner(&before, &entry.owner_display(pane.owner_display));
            changed = true;
        }
    }
    let detail = &mut app.details[target.pane];
    if detail.path.as_ref() == Some(&target.key.path)
        && let Some(entry) = detail.entry.as_mut()
        && target.key.matches(entry)
        && entry.windows_owner != owner
    {
        entry.windows_owner = owner;
        changed = true;
    }
    changed
}
