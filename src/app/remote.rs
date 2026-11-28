use super::{App, Pane};
use crate::fs::Entry;
use russh_sftp::{client::fs::ReadDir, protocol::SftpString};

pub(super) struct Location {
    path: SftpString,
    pub cursor: Option<SftpString>,
}

impl App {
    pub(crate) fn install_sftp(&mut self, path: SftpString, entries: ReadDir) {
        let entries = entries
            .enumerate()
            .map(|(order, entry)| Entry::from_sftp(entry, order))
            .collect();
        self.install_remote_rows(path, entries);
    }

    fn install_remote_rows(&mut self, path: SftpString, entries: Vec<Entry>) {
        let mut pane = Pane::new(Default::default());
        // Keep the existing right pane's independent view preferences.
        let right = &mut self.panes[1];
        pane.profile.clone_from(&right.profile);
        pane.profile_modified = right.profile_modified;
        pane.remote = Some(Location { path, cursor: None });
        pane.listing = false;
        pane.listing_installs = 1;
        pane.entries = entries;
        crate::view::ViewSettings::capture(right).apply(&mut pane);
        *right = pane;
        self.dual = true;
        self.maximized = false;
    }
}

#[cfg(test)]
mod tests;

impl Pane {
    /// Whether this pane contains a read-only SFTP listing.
    pub fn is_remote(&self) -> bool {
        self.remote.is_some()
    }

    /// Directory path as sent by SFTP, without native path interpretation.
    pub fn remote_path(&self) -> Option<&[u8]> {
        self.remote
            .as_ref()
            .map(|location| location.path.as_bytes())
    }
}
