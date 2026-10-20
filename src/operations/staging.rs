use super::{
    Outcome,
    plan::invalid,
    safety::{self, Parents, Result},
};
use crate::fs::Entry;
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

pub(super) struct OwnedStage {
    pub path: PathBuf,
    pub file: Option<File>,
    root: Entry,
    entry: Entry,
    parents: Parents,
}

impl OwnedStage {
    pub fn claim_event(&self) -> super::Event {
        super::Event::OwnedStage {
            root: Box::new(self.root.clone()),
            payload: Box::new(self.entry.clone()),
        }
    }

    pub fn recovery(
        &self,
        source: &Entry,
        destination: &Path,
        previous: Option<&Entry>,
        guards: &[Parents; 2],
    ) -> super::Recovery {
        super::Recovery {
            source: source.clone(),
            destination: destination.to_owned(),
            previous: previous.cloned(),
            committed: None,
            staging: Some(super::recovery::Staging {
                root: self.root.clone(),
                payload: self.entry.clone(),
            }),
            guards: guards.clone(),
        }
    }

    pub fn seal(&mut self) -> Result<()> {
        safety::owned_object(&self.entry)?;
        self.entry = crate::fs::inspect(&self.path);
        Ok(())
    }
    pub fn finish(self, transfer: Result<Entry>) -> Result<Entry> {
        let recovery = self.root.path.clone();
        if transfer
            .as_ref()
            .is_err_and(|error| error.outcome == Outcome::OutcomeUnknown)
        {
            return Err(safety::Failure {
                outcome: Outcome::OutcomeUnknown,
                detail: format!(
                    "Outcome unknown; staging quarantined at {}",
                    recovery.display()
                ),
            });
        }
        if let Err(error) = self.cleanup() {
            return Err(safety::Failure {
                outcome: Outcome::OutcomeUnknown,
                detail: format!(
                    "Cleanup unverified; staging retained at {}: {}",
                    recovery.display(),
                    error.detail
                ),
            });
        }
        transfer
    }

    pub fn create(destination: &Path, link: Option<&Entry>) -> Result<Self> {
        let parent = destination
            .parent()
            .ok_or_else(|| invalid("Destination lacks parent"))?;
        let root = tempfile::Builder::new()
            .prefix(".tfm-stage-")
            .tempdir_in(parent)?
            .keep();
        let root = crate::fs::inspect(&root);
        let path = root.path.join("payload");
        let file = if let Some(source) = link {
            create_link(source, &path)?;
            None
        } else {
            Some(
                OpenOptions::new()
                    .write(true)
                    .read(true)
                    .create_new(true)
                    .open(&path)?,
            )
        };
        Ok(Self {
            entry: crate::fs::inspect(&path),
            parents: Parents::capture(&path)?,
            path,
            file,
            root,
        })
    }
    pub fn validate(&self) -> Result<()> {
        self.parents.validate()?;
        safety::revalidate(&self.root, false)?;
        safety::revalidate(&self.entry, true)
    }
    pub fn commit(&mut self, destination: &Path, replace: bool) -> Result<()> {
        self.validate()?;
        if let Some(file) = self.file.take() {
            file.sync_all()?;
            #[cfg(unix)]
            nix::unistd::close(file).map_err(io::Error::from)?;
        }
        let renamed = if replace {
            fs::rename(&self.path, destination)
        } else {
            rename_new(&self.path, destination)
        };
        if let Err(error) = renamed {
            if safety::owned_object(&self.entry).is_ok() {
                return Err(error.into());
            }
            return Err(safety::Failure {
                outcome: Outcome::OutcomeUnknown,
                detail: format!(
                    "Commit unacknowledged; staging/final state requires reconciliation: {error}"
                ),
            });
        }
        Ok(())
    }
    pub fn cleanup(self) -> Result<()> {
        self.parents.validate()?;
        safety::revalidate(&self.root, false)?;
        if safety::existing(&self.path)?.is_some() {
            safety::owned_object(&self.entry)?;
            remove_leaf(&self.path)?;
        }
        fs::remove_dir(&self.root.path)?;
        Ok(())
    }

    pub fn committed(&self, destination: &Path) -> Result<Entry> {
        let mut expected = self.entry.clone();
        expected.path = destination.to_owned();
        safety::owned_object(&expected)?;
        #[cfg(windows)]
        restore_readonly(&mut expected)?;
        safety::revalidate(&expected, true)?;
        Ok(crate::fs::inspect(destination))
    }
}

#[cfg(windows)]
fn restore_readonly(expected: &mut Entry) -> Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    let readonly = u32::from_str_radix(expected.attributes.trim_start_matches("0x"), 16)
        .map_err(io::Error::other)?
        & 1
        != 0;
    let file = OpenOptions::new()
        .access_mode(0x0000_0180)
        .custom_flags(0x0020_0000)
        .open(&expected.path)?;
    safety::opened_object(expected, &file)?;
    let mut permissions = file.metadata()?.permissions();
    permissions.set_readonly(readonly);
    file.set_permissions(permissions)?;
    let after = crate::fs::inspect(&expected.path);
    let old = u32::from_str_radix(expected.attributes.trim_start_matches("0x"), 16)
        .map_err(io::Error::other)?;
    let new = u32::from_str_radix(after.attributes.trim_start_matches("0x"), 16)
        .map_err(io::Error::other)?;
    // tempfile persistence normalizes Archive/Temporary/Normal on our stage.
    if (old ^ new) & !0x1a0 != 0 {
        return Err(safety::changed(
            "Committed attributes differ from owned stage",
        ));
    }
    expected.attributes = after.attributes;
    Ok(())
}

pub(super) fn open_source(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("Source is no longer a regular file"));
    }
    Ok(file)
}

pub(super) fn rename_new(source: &Path, destination: &Path) -> io::Result<()> {
    let mut path = tempfile::TempPath::try_from_path(source)?;
    path.disable_cleanup(true);
    path.persist_noclobber(destination)
        .map_err(|error| error.error)
}

pub(super) fn remove_leaf(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt;
        if fs::symlink_metadata(path)?.file_type().is_symlink_dir() {
            return fs::remove_dir(path);
        }
    }
    fs::remove_file(path)
}

fn create_link(source: &Entry, destination: &Path) -> io::Result<()> {
    let target = source
        .link_target
        .as_ref()
        .ok_or_else(|| invalid("Link target unavailable"))?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, destination)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt;
        let kind = fs::symlink_metadata(&source.path)?.file_type();
        if kind.is_symlink_dir() {
            std::os::windows::fs::symlink_dir(target, destination)
        } else {
            std::os::windows::fs::symlink_file(target, destination)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_identity_is_the_owned_stage_not_a_fresh_unrelated_destination() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("final");
        let mut stage = OwnedStage::create(&destination, None).unwrap();
        stage.seal().unwrap();
        stage.commit(&destination, false).unwrap();
        fs::rename(&destination, root.path().join("committed-original")).unwrap();
        fs::write(&destination, "external").unwrap();
        assert!(stage.committed(&destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"external");
        stage.cleanup().unwrap();
    }
}
