use crate::{settings::Configuration, startup::Startup};
use russh_sftp::{client::fs::ReadDir, protocol::SftpString};

/// Open the browser on the current local directory and a read-only SFTP snapshot.
///
/// # Errors
/// Reports local startup, terminal, or configuration errors.
pub fn run_sftp_preview(path: SftpString, entries: ReadDir) -> Result<(), String> {
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
    super::browse(app, settings, &mut config)
}
