use crate::{settings::Configuration, ssh::Session, startup::Startup};
use russh_sftp::{client::fs::ReadDir, protocol::SftpString};
use std::sync::Arc;

/// Open the browser on the current local directory and a live read-only SFTP pane.
///
/// # Errors
/// Reports local startup, terminal, or configuration errors.
pub fn run_sftp_preview(
    session: Arc<Session>,
    path: SftpString,
    entries: ReadDir,
) -> Result<(), String> {
    let mut config = Configuration::open()?;
    let settings = config.preferences();
    // An explicit local path bypasses navigation-session restoration.
    let local = Startup {
        paths: vec![".".into()],
        ..Default::default()
    };
    let mut app = local.prepare(&settings)?;
    super::require_terminal()?;
    app.install_sftp(path, entries);
    super::browse(app, settings, &mut config, Some(session))
}
