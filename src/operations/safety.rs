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
    if source_path == destination_path
        || (source.may_recurse() && destination_path.starts_with(&source_path))
    {
        return Err(invalid("Source/destination overlap or self-copy refused").into());
    }
    if let Some(target) = existing(destination)?
        && source.identity.as_ref().and_then(|id| id.object).is_some()
        && source.identity.as_ref().and_then(|id| id.object)
            == target.identity.as_ref().and_then(|id| id.object)
    {
        return Err(invalid("Source/destination are aliases of the same object").into());
    }
    Ok(())
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
