#![cfg(target_os = "linux")]

use std::{
    fs, io,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
};
use terminal_file_manager::{
    fs::{Entry, EntryKind},
    operations::{self, Decision, Operation, Outcome, Plan, trash::LinuxReceipt},
};

fn same_object(expected: &Entry, path: &Path) -> bool {
    let fresh = terminal_file_manager::fs::inspect(path);
    let owner = |entry: &Entry| entry.owner.as_ref().map(|owner| (owner.uid, owner.gid));
    expected.kind == fresh.kind
        && expected.attributes == fresh.attributes
        && owner(expected) == owner(&fresh)
        && expected
            .identity
            .as_ref()
            .zip(fresh.identity.as_ref())
            .is_some_and(|(a, b)| {
                a.object.is_some() && a.object == b.object && a.created == b.created
            })
}

fn present(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub struct ReceiptFixture {
    pub receipt: LinuxReceipt,
    payload: Entry,
    info: Entry,
    original: PathBuf,
    parent: Entry,
}

impl ReceiptFixture {
    pub fn new(receipt: LinuxReceipt, original: PathBuf) -> Self {
        Self {
            payload: terminal_file_manager::fs::inspect(&receipt.path),
            info: terminal_file_manager::fs::inspect(&receipt.info),
            parent: terminal_file_manager::fs::inspect(original.parent().unwrap()),
            receipt,
            original,
        }
    }
    pub fn cleanup(&self) -> io::Result<()> {
        if !same_object(&self.parent, self.original.parent().unwrap()) {
            return Err(io::Error::other("Fixture restore parent changed"));
        }
        self.restore()?;
        self.remove_info()
    }

    fn restore(&self) -> io::Result<()> {
        if present(&self.receipt.path)? {
            if !same_object(&self.payload, &self.receipt.path)
                || terminal_file_manager::fs::inspect(&self.receipt.path).identity
                    != self.payload.identity
            {
                return Err(io::Error::other("Fixture receipt payload changed"));
            }
            let mut plan = Plan::paths(
                Operation::Rename,
                vec![self.receipt.path.clone()],
                Some(self.original.clone()),
            )?;
            plan.operands[0] = self.payload.clone();
            let record =
                operations::execute(&plan, Default::default(), &false.into(), |_| Decision::Skip);
            if record.entries[0].outcome != Outcome::Completed {
                return Err(io::Error::other(
                    "Fixture restore refused; payload retained",
                ));
            }
        }
        if !same_object(&self.payload, &self.original) {
            return Err(io::Error::other("Restored fixture identity unverified"));
        }
        Ok(())
    }

    fn remove_info(&self) -> io::Result<()> {
        if present(&self.receipt.info)? {
            if !same_object(&self.info, &self.receipt.info)
                || terminal_file_manager::fs::inspect(&self.receipt.info).identity
                    != self.info.identity
            {
                return Err(io::Error::other("Fixture receipt metadata changed"));
            }
            fs::remove_file(&self.receipt.info)?;
        }
        Ok(())
    }
}

impl Drop for ReceiptFixture {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[derive(Default)]
pub struct OwnedStore {
    directories: Vec<Entry>,
}

impl OwnedStore {
    pub fn prepare(top: &Path) -> io::Result<Self> {
        let uid = nix::unistd::geteuid().as_raw();
        let mut stores = vec![top.join(format!(".Trash-{uid}"))];
        let shared = top.join(".Trash");
        if valid_shared(&shared, uid) {
            stores.push(shared.join(uid.to_string()));
        }
        let mut owned = Self::default();
        for store in stores {
            owned.store(&store, uid)?;
        }
        Ok(owned)
    }

    fn store(&mut self, store: &Path, uid: u32) -> io::Result<()> {
        self.create(store)?;
        let meta = fs::symlink_metadata(store)?;
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || meta.uid() != uid
            || meta.mode() & 0o777 != 0o700
        {
            return Err(io::Error::other("Unsafe pre-existing volume Trash fixture"));
        }
        self.create(&store.join("files"))?;
        self.create(&store.join("info"))
    }
    pub fn create(&mut self, path: &Path) -> io::Result<()> {
        match fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => self
                .directories
                .push(terminal_file_manager::fs::inspect(path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        Ok(())
    }
    pub fn cleanup(&self) -> io::Result<()> {
        for entry in self.directories.iter().rev() {
            if !present(&entry.path)? {
                continue;
            }
            if entry.kind != EntryKind::Directory || !same_object(entry, &entry.path) {
                return Err(io::Error::other(
                    "Fixture-created directory identity changed",
                ));
            }
            fs::remove_dir(&entry.path)?;
        }
        Ok(())
    }
}

fn valid_shared(path: &Path, uid: u32) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| {
        meta.is_dir()
            && !meta.file_type().is_symlink()
            && meta.mode() & 0o1000 != 0
            && [0, uid].contains(&meta.uid())
    })
}

impl Drop for OwnedStore {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
