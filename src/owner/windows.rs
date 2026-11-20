//! Cached Windows owner state. Native queries belong exclusively to workers.
use super::OwnerDisplay;

pub mod runtime;

// Maintainer-approved exception for this read-only adapter. Keep pointer and
// allocation ownership within native.rs; reassess when adding another API.
#[allow(unsafe_code, reason = "Audited read-only Windows owner/SID adapter")]
mod native;
pub use native::{read_sid, resolve_name};

/// Name translation can fail independently of retrieving a file's SID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Name {
    Pending,
    Resolved(String),
    Unmapped,
    Failed(String),
}

/// Preserve raw SID even while domain account translation is outstanding.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Ownership {
    #[default]
    Pending,
    Sid {
        sid: String,
        name: Name,
    },
    Unavailable(String),
}

impl Ownership {
    /// Cached-only presentation shared by listing, sidebar and popup.
    pub fn display(&self, mode: OwnerDisplay) -> String {
        match self {
            Self::Pending => "resolving".into(),
            Self::Unavailable(_) => "unavailable".into(),
            Self::Sid { sid, name } => match (mode, name) {
                (OwnerDisplay::Numeric, _) | (_, Name::Unmapped) => sid.clone(),
                (_, Name::Resolved(name)) => name.clone(),
                (_, Name::Pending) => format!("{sid} (resolving)"),
                (_, Name::Failed(_)) => format!("{sid} (lookup failed)"),
            },
        }
    }

    /// Report the actual failure without replacing any available numeric SID.
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Unavailable(error)
            | Self::Sid {
                name: Name::Failed(error),
                ..
            } => Some(error),
            _ => None,
        }
    }
}
