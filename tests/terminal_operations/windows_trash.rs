//! Windows F8 approval through the actual executable and ConPTY.
use super::*;
use terminal_file_manager::fs::inspect;

fn object(path: &Path) -> (u64, u64) {
    inspect(path).identity.unwrap().object.unwrap()
}

#[test]
fn windows_trash_enter_cancels_tfm_approval_without_changing_identity() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("tfm-trash-cancel.txt");
    fs::write(&source, b"retained").unwrap();
    let before = object(&source);
    drive(
        command(root.path(), root.path(), None),
        &[
            ("tfm-trash-cancel.txt", "\x1b[F\x1b[19~"),
            ("T Trash", "\rq"),
        ],
        false,
    );
    assert_eq!(object(&source), before);
    assert_eq!(fs::read(source).unwrap(), b"retained");
}

#[test]
fn windows_trash_t_approval_recycles_from_the_actual_executable() {
    if std::env::var("RUNNER_ENVIRONMENT").as_deref() != Ok("github-hosted")
        && std::env::var("TFM_WINDOWS_TRASH_NATIVE").as_deref() != Ok("owned-fixtures")
    {
        eprintln!("NOT RUN: owned Windows Recycle Bin fixture requires explicit native opt-in");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("files");
    fs::create_dir(&folder).unwrap();
    let source = folder.join("tfm-trash-approve.txt");
    let anchor = root.path().join("recovery-anchor");
    fs::write(&source, b"recoverable executable fixture").unwrap();
    fs::hard_link(&source, &anchor).unwrap();
    let before = object(&source);
    drive(
        command(root.path(), &folder, None),
        &[
            ("tfm-trash-approve.txt", "\x1b[B\x1b[19~"),
            ("1 file · 30 B", "T"),
            ("sent to Trash", "q"),
        ],
        false,
    );
    assert_eq!(
        fs::symlink_metadata(source).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(object(&anchor), before);
    assert_eq!(fs::read(anchor).unwrap(), b"recoverable executable fixture");
}
