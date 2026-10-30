use super::Pane;
use std::path::PathBuf;

/// One pending cursor intent; later user activity takes precedence.
pub(super) struct CreationFocus {
    id: u64,
    path: PathBuf,
    cursor: Option<PathBuf>,
    revision: u64,
    completed_generation: Option<u64>,
}

impl CreationFocus {
    pub(super) fn current(&self, pane: &Pane) -> bool {
        self.revision == pane.selection_revision && self.cursor == pane.cursor
    }
}

impl Pane {
    pub(super) fn take_creation(&mut self, continuous: bool) -> Option<CreationFocus> {
        self.creation
            .take()
            .filter(|intent| continuous && intent.current(self))
    }

    pub(crate) fn track_creation(&mut self, id: u64, path: PathBuf) {
        if self.pending.is_none() && path.parent() == Some(self.path.as_path()) {
            self.creation = Some(CreationFocus {
                id,
                path,
                cursor: self.cursor.clone(),
                revision: self.selection_revision,
                completed_generation: None,
            });
        }
    }

    pub(crate) fn finish_creation(&mut self, id: u64, success: bool) {
        if self.creation.as_ref().is_some_and(|intent| intent.id == id) {
            self.creation = self
                .creation
                .take()
                .filter(|intent| success && intent.current(self));
            if let Some(intent) = &mut self.creation {
                intent.completed_generation = Some(self.generation);
            }
        }
    }

    pub(super) fn restore_creation(&mut self, intent: Option<CreationFocus>) {
        let Some(mut intent) = intent else { return };
        if intent
            .completed_generation
            .is_some_and(|generation| self.generation > generation)
        {
            if self
                .visible
                .iter()
                .any(|i| self.entries[*i].path == intent.path)
            {
                self.cursor = Some(intent.path);
                self.reset_range();
            }
        } else {
            intent.revision = self.selection_revision;
            intent.cursor = self.cursor.clone();
            self.creation = Some(intent);
        }
    }
}
