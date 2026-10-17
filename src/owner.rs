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
    fn lsplus_user_and_group_resolution_retains_native_names() {
        assert!(matches!(get_username(0).as_str(), "root" | "0"));
        // macOS names GID 0 wheel rather than root; retain native account names.
        assert!(matches!(get_groupname(0).as_str(), "root" | "wheel" | "0"));
        if let Ok(Some(user)) = nix::unistd::User::from_uid(u32::MAX.into()) {
            assert_eq!(get_username(u32::MAX), user.name);
        }
        if let Ok(Some(group)) = nix::unistd::Group::from_gid(u32::MAX.into()) {
            assert_eq!(get_groupname(u32::MAX), group.name);
        }
    }

    #[test]
    fn unresolved_user_retains_numeric_fallback() {
        // Require lookup failure instead of assuming a reserved ID is absent.
        let uid = (0..=u32::MAX)
            .rev()
            .find(|uid| !matches!(nix::unistd::User::from_uid((*uid).into()), Ok(Some(_))))
            .expect("a UID that native account lookup cannot resolve");
        assert_eq!(get_username(uid), uid.to_string());
    }

    #[test]
    fn unresolved_group_retains_numeric_fallback() {
        // macOS resolves u32::MAX as nogroup; require an actually absent group.
        let gid = (0..=u32::MAX)
            .rev()
            .find(|gid| !matches!(nix::unistd::Group::from_gid((*gid).into()), Ok(Some(_))))
            .expect("a GID that native group lookup cannot resolve");
        assert_eq!(get_groupname(gid), gid.to_string());
    }
}
