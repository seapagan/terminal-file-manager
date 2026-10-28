use super::{
    Outcome,
    plan::{invalid, same_object},
};
use crate::fs::{Entry, EntryKind};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(super) struct Failure {
    pub outcome: Outcome,
    pub detail: String,
}
pub(super) type Result<T> = std::result::Result<T, Failure>;

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Self {
            outcome: if error.kind() == io::ErrorKind::Interrupted {
                Outcome::Cancelled
            } else {
                Outcome::Failed
            },
            detail: error.to_string(),
        }
    }
}

pub(super) fn changed(message: &str) -> Failure {
    Failure {
        outcome: Outcome::Changed,
        detail: message.into(),
    }
}

pub(super) fn revalidate(expected: &Entry, contents: bool) -> Result<()> {
    let fresh = crate::fs::inspect(&expected.path);
    let same = match (&expected.identity, &fresh.identity) {
        (Some(a), Some(b)) => {
            if contents {
                a == b
            } else {
                same_object(a, b)
            }
        }
        _ => false,
    };
    let ownership = |entry: &Entry| entry.owner.as_ref().map(|owner| (owner.uid, owner.gid));
    if fresh.kind != expected.kind
        || !same
        || fresh.link_target != expected.link_target
        || fresh.attributes != expected.attributes
        || ownership(&fresh) != ownership(expected)
    {
        return Err(changed("Source/object changed; fresh review required"));
    }
    Ok(())
}

pub(super) fn owned_object(expected: &Entry) -> Result<()> {
    let fresh = crate::fs::inspect(&expected.path);
    if fresh.kind == expected.kind
        && fresh.link_target == expected.link_target
        && expected
            .identity
            .as_ref()
            .zip(fresh.identity.as_ref())
            .is_some_and(|(a, b)| same_object(a, b))
    {
        Ok(())
    } else {
        Err(changed("Owned object identity changed"))
    }
}

pub(super) fn opened_object(expected: &Entry, file: &fs::File) -> Result<()> {
    #[cfg(unix)]
    let object = {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata()?;
        (metadata.dev(), metadata.ino())
    };
    #[cfg(windows)]
    let object = {
        let information = winapi_util::file::information(file)?;
        (information.volume_serial_number(), information.file_index())
    };
    if expected.identity.as_ref().and_then(|id| id.object) != Some(object) {
        return Err(changed("Opened handle differs from approved native object"));
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub(super) struct Parents {
    raw: PathBuf,
    resolved: PathBuf,
    chain: Vec<Entry>,
}

impl Parents {
    pub fn capture(path: &Path) -> Result<Self> {
        let raw = std::path::absolute(path)?
            .parent()
            .ok_or_else(|| invalid("Root mutation refused"))?
            .to_owned();
        let resolved = fs::canonicalize(&raw)?;
        let mut chain = Vec::new();
        for path in resolved.ancestors() {
            let entry = crate::fs::inspect(path);
            if entry.kind != EntryKind::Directory
                || entry.identity.as_ref().and_then(|id| id.object).is_none()
            {
                return Err(changed("Parent directory identity unavailable"));
            }
            chain.push(entry);
        }
        Ok(Self {
            raw,
            resolved,
            chain,
        })
    }
    pub fn validate(&self) -> Result<()> {
        if fs::canonicalize(&self.raw)? != self.resolved {
            return Err(changed("Parent namespace changed"));
        }
        for entry in &self.chain {
            revalidate(entry, false)?;
        }
        Ok(())
    }
}

pub(super) fn destination_matches(path: &Path, expected: Option<&Entry>) -> Result<()> {
    match (fs::symlink_metadata(path), expected) {
        (Err(error), None) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        (Ok(_), Some(entry)) => revalidate(entry, true),
        _ => Err(changed(
            "Destination changed after approval/conflict decision",
        )),
    }
}

pub(super) fn existing(path: &Path) -> Result<Option<Entry>> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(Some(crate::fs::inspect(path))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn reject_overlap(source: &Entry, destination: &Path) -> Result<()> {
    let source_path = super::locks::namespace(&source.path)?;
    let destination_path = super::locks::namespace(destination)?;
    if source_path == destination_path {
        let reason = match (source.path == destination, source.may_recurse()) {
            (false, _) => "Source and destination refer to the same item.",
            (true, true) => "Source and destination are the same folder.",
            (true, false) => "Source and destination are the same.",
        };
        return Err(invalid(reason).into());
    }
    if source.may_recurse() && destination_path.starts_with(&source_path) {
        return Err(invalid("Destination is inside the source folder.").into());
    }
    let source_object = source.identity.as_ref().and_then(|id| id.object);
    if let Some(target) = existing(destination)?
        && source_object
            .is_some_and(|object| target.identity.as_ref().and_then(|id| id.object) == Some(object))
    {
        return Err(invalid("Source and destination refer to the same item.").into());
    }
    Ok(())
}

pub(super) fn same_entry_rename(source: &Entry, destination: &Path) -> Result<bool> {
    let Some(target) = existing(destination)? else {
        return Ok(false);
    };
    if !native_alias(source, &target) || source.path.file_name() == destination.file_name() {
        return Ok(false);
    }
    let Some(parent) = rename_parent(source, destination)? else {
        return Ok(false);
    };
    let selected = selected_name_only(&parent.path, &source.path, destination)?;
    for entry in [&parent, source, &target] {
        revalidate(entry, true)?;
    }
    Ok(selected)
}

fn native_alias(a: &Entry, b: &Entry) -> bool {
    a.identity
        .as_ref()
        .zip(b.identity.as_ref())
        .is_some_and(|(a, b)| a.object.is_some() && same_object(a, b))
}

fn rename_parent(source: &Entry, destination: &Path) -> Result<Option<Entry>> {
    let source_parent = source
        .path
        .parent()
        .ok_or_else(|| invalid("Rename parent required"))?;
    let target_parent = destination
        .parent()
        .ok_or_else(|| invalid("Rename parent required"))?;
    let parent = crate::fs::inspect(source_parent);
    revalidate(&parent, false)?;
    let other = crate::fs::inspect(target_parent);
    if !native_alias(&parent, &other) {
        return Ok(None);
    }
    Ok(Some(parent))
}

fn selected_name_only(parent: &Path, source: &Path, destination: &Path) -> io::Result<bool> {
    let (mut selected, mut separate_target) = (false, false);
    for entry in fs::read_dir(parent)? {
        let name = entry?.file_name();
        selected |= Some(name.as_os_str()) == source.file_name();
        separate_target |= Some(name.as_os_str()) == destination.file_name();
    }
    Ok(selected && !separate_target)
}

pub(super) fn acknowledged_remove(
    entry: &Entry,
    remove: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<()> {
    if let Err(error) = remove(&entry.path) {
        if revalidate(entry, false).is_ok() {
            return Err(error.into());
        }
        return Err(Failure {
            outcome: Outcome::OutcomeUnknown,
            detail: format!(
                "Source removal unacknowledged; source may already be removed or replaced: {error}"
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opened_handle_must_match_the_approved_native_object() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source");
        let other = root.path().join("other");
        fs::write(&path, "same").unwrap();
        fs::write(&other, "same").unwrap();
        let expected = crate::fs::inspect(&path);
        opened_object(&expected, &fs::File::open(&path).unwrap()).unwrap();
        let error = opened_object(&expected, &fs::File::open(other).unwrap()).unwrap_err();
        assert_eq!(error.outcome, Outcome::Changed);
        assert_eq!(fs::read(path).unwrap(), b"same");
    }

    #[test]
    fn missing_native_identity_never_invents_an_alias_or_bypasses_namespace_checks() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source");
        let other = root.path().join("other");
        fs::write(&path, "owned").unwrap();
        fs::write(&other, "separate").unwrap();
        let mut source = crate::fs::inspect(&path);
        source.identity.as_mut().unwrap().object = None;
        for identity in [source.identity.clone(), None] {
            source.identity = identity;
            reject_overlap(&source, &other).unwrap();
            let failure = reject_overlap(&source, &path).unwrap_err();
            assert_eq!(failure.outcome, Outcome::Failed);
            assert_eq!(failure.detail, "Source and destination are the same.");
        }
        assert_eq!(fs::read(path).unwrap(), b"owned");
        assert_eq!(fs::read(other).unwrap(), b"separate");
    }

    #[test]
    fn lost_removal_response_never_claims_source_preservation_or_reissues_removal() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source");
        fs::write(&path, "original").unwrap();
        let entry = crate::fs::inspect(&path);
        let result = acknowledged_remove(&entry, |path| {
            fs::remove_file(path)?;
            fs::write(path, "external replacement")?;
            Err(io::Error::other("lost response"))
        })
        .unwrap_err();
        assert_eq!(result.outcome, Outcome::OutcomeUnknown);
        assert_eq!(fs::read(path).unwrap(), b"external replacement");
    }
}
