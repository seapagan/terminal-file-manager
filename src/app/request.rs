//! Typed targets for bounded browser worker requests.
use std::path::PathBuf;

/// A bounded worker request carries identity independently of display text.
#[derive(Clone, Debug)]
pub struct Request {
    pub pane: usize,
    pub generation: u64,
    pub path: Option<PathBuf>,
    pub kind: RequestKind,
}

/// Independent request lanes keep slow counts from starving listings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestKind {
    Listing,
    SftpListing(russh_sftp::protocol::SftpString),
    SftpActivate(russh_sftp::protocol::SftpString),
    /// Parent listing with a lexical source to inspect if it has never been listed.
    ParentListing(Option<PathBuf>),
    /// Best-effort no-follow identity lookup for a pending or completed Parent request.
    ParentIdentity,
    Metadata,
    Totals,
}

#[derive(PartialEq, Eq)]
pub(crate) enum NavigationTarget {
    Native(PathBuf),
    Sftp(russh_sftp::protocol::SftpString),
}

impl Request {
    /// A native operand, absent for SFTP listing requests.
    pub fn native_path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    /// A path for consumers of native-only requests.
    pub fn local_path(&self) -> &PathBuf {
        self.native_path().expect("native request")
    }
}

impl RequestKind {
    pub(crate) fn sftp_path(&self) -> Option<&russh_sftp::protocol::SftpString> {
        match self {
            Self::SftpListing(path) | Self::SftpActivate(path) => Some(path),
            _ => None,
        }
    }
}
