#![cfg(target_os = "linux")]

use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

pub fn second_filesystem(source: &Path) -> Option<tempfile::TempDir> {
    let device = fs::metadata(source).unwrap().dev();
    let candidates = [
        PathBuf::from("/dev/shm"),
        PathBuf::from(format!("/run/user/{}", nix::unistd::geteuid())),
        PathBuf::from("/run"),
        PathBuf::from("/var/tmp"),
        PathBuf::from("/tmp"),
    ];
    for base in &candidates {
        if fs::metadata(base).is_ok_and(|metadata| metadata.dev() != device)
            && let Ok(root) = tempfile::tempdir_in(base)
            && fs::metadata(root.path()).unwrap().dev() != device
        {
            return Some(root);
        }
    }
    assert_ne!(
        std::env::var("RUNNER_ENVIRONMENT").as_deref(),
        Ok("github-hosted"),
        "Hosted Linux must exercise a real second filesystem; checked {candidates:?}"
    );
    eprintln!(
        "SKIP: no writable second filesystem for an owned native fixture; source device {device}, checked {candidates:?}"
    );
    None
}

pub fn volume_root(path: &Path) -> PathBuf {
    let mut top = fs::canonicalize(path).unwrap();
    let device = fs::metadata(&top).unwrap().dev();
    while let Some(parent) = top.parent() {
        if fs::metadata(parent).unwrap().dev() != device {
            break;
        }
        top = parent.to_owned();
    }
    top
}
