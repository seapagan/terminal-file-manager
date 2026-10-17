//! Unix account resolution adapted from lsplus d30ea4a (MIT, Grant Ramsay).

/// Names and numeric IDs are independent per-pane view preferences.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnerDisplay {
    #[default]
    Names,
    Numeric,
}

/// Preserve raw UID/GID alongside worker-resolved display names.
#[derive(Clone, Debug)]
pub struct Ownership {
    pub uid: u32,
    pub gid: u32,
    pub user: String,
    pub group: String,
}

impl Ownership {
    /// Formatting cached values performs no account lookup or filesystem I/O.
    pub fn display(&self, mode: OwnerDisplay) -> String {
        match mode {
            OwnerDisplay::Names => format!("{}:{}", self.user, self.group),
            OwnerDisplay::Numeric => format!("{}:{}", self.uid, self.gid),
        }
    }
}

#[cfg(unix)]
pub(crate) fn lookup(uid: u32, gid: u32) -> Ownership {
    Ownership {
        uid,
        gid,
        user: get_username(uid),
        group: get_groupname(gid),
    }
}

/// Look up a username, falling back to the numeric UID.
#[cfg(unix)]
fn get_username(uid: u32) -> String {
    match nix::unistd::User::from_uid(uid.into()) {
        Ok(Some(user)) => user.name,
        _ => uid.to_string(),
    }
}

/// Look up a group name, falling back to the numeric GID.
#[cfg(unix)]
fn get_groupname(gid: u32) -> String {
    match nix::unistd::Group::from_gid(gid.into()) {
        Ok(Some(group)) => group.name,
        _ => gid.to_string(),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn lsplus_user_and_group_resolution_retains_numeric_fallbacks() {
        assert!(matches!(get_username(0).as_str(), "root" | "0"));
        assert_eq!(get_username(u32::MAX), u32::MAX.to_string());
        // macOS names GID 0 wheel rather than root; retain native account names.
        assert!(matches!(get_groupname(0).as_str(), "root" | "wheel" | "0"));
        assert_eq!(get_groupname(u32::MAX), u32::MAX.to_string());
    }
}
