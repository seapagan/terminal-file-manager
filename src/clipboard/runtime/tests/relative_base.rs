use super::*;
use crate::{app::App, path_copy::Outcome, session::SessionStore};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn selected_app(root: &Path, base: &Path, restored: bool) -> App {
    let mut app = app(root);
    app.relative_path_base = base.into();
    if restored {
        let store = SessionStore {
            path: root.join("session.toml"),
        };
        store.save(&app).unwrap();
        app = store.load().unwrap().restore(&Default::default()).unwrap();
        let listing = crate::fs::list(root, &Default::default(), &Default::default()).unwrap();
        app.complete_listing(0, 1, Ok(listing));
    }
    app.panes[0].cursor = Some(root.join(TEXT));
    assert_eq!(app.relative_path_base, base);
    app
}

fn copy_request(app: &mut App, format: &'static str) -> CopyRequest {
    let Outcome::Copy(request) = completion_as(app, "highlighted", format) else {
        panic!("copy request")
    };
    request
}

fn reject_without_copying(app: &mut App) -> String {
    let selected = app.relative_path_base.clone();
    let attempted = Arc::new(AtomicBool::new(false));
    let probe = attempted.clone();
    let mut runtime = Runtime::default();
    runtime
        .start_using(copy_request(app, "relative"), move |_| {
            probe.store(true, Ordering::Relaxed);
            Ok(Delivery::Copied)
        })
        .unwrap();
    let mut terminal = Vec::new();
    let error = wait(&mut runtime, &mut terminal).unwrap_err();
    assert!(error.contains("relative-path base"), "{error}");
    assert!(error.contains(&selected.display().to_string()), "{error}");
    assert!(!attempted.load(Ordering::Relaxed));
    assert!(terminal.is_empty());
    assert_eq!(app.relative_path_base, selected);
    error
}

fn copy_names_and_full_paths(app: &mut App, root: &Path) {
    for (format, expected) in [
        ("name", TEXT.to_owned()),
        ("full", root.join(TEXT).to_str().unwrap().to_owned()),
    ] {
        let selected = app.relative_path_base.clone();
        let mut runtime = Runtime::default();
        runtime
            .start_using(copy_request(app, format), move |text| {
                assert_eq!(text, expected);
                Ok(Delivery::Copied)
            })
            .unwrap();
        assert!(
            wait(&mut runtime, &mut Vec::new())
                .unwrap()
                .starts_with("Copied")
        );
        assert_eq!(app.relative_path_base, selected);
    }
}

#[test]
fn valid_selected_and_restored_bases_produce_lexical_relative_text() {
    for restored in [false, true] {
        let root = root();
        let base = root.path().join("base 界");
        std::fs::create_dir(&base).unwrap();
        let mut app = selected_app(root.path(), &base, restored);
        let mut runtime = Runtime::default();
        runtime
            .start_using(copy_request(&mut app, "relative"), |text| {
                assert_eq!(text, Path::new("..").join(TEXT).to_str().unwrap());
                Ok(Delivery::Copied)
            })
            .unwrap();
        assert!(wait(&mut runtime, &mut Vec::new()).is_ok());
        assert_eq!(app.relative_path_base, base);
    }
}

#[test]
fn deleted_and_file_bases_reject_relative_only_including_restored_sessions() {
    for restored in [false, true] {
        for file in [false, true] {
            let root = root();
            let base = root.path().join("base 界");
            std::fs::create_dir(&base).unwrap();
            std::fs::remove_dir(&base).unwrap();
            if file {
                std::fs::write(&base, "not a directory").unwrap();
            }
            let mut app = selected_app(root.path(), &base, restored);
            let saved = std::fs::read(root.path().join("session.toml")).ok();
            let native = std::fs::read_dir(&base).unwrap_err().to_string();
            let error = reject_without_copying(&mut app);
            assert!(error.contains(&native), "{error}");
            copy_names_and_full_paths(&mut app, root.path());
            assert_eq!(std::fs::read(root.path().join("session.toml")).ok(), saved);
        }
    }
}

#[cfg(unix)]
#[test]
fn unreadable_selected_and_restored_bases_fail_without_changing_the_base_or_clipboard() {
    use std::os::unix::fs::PermissionsExt;
    for restored in [false, true] {
        let root = root();
        let base = root.path().join("private-base");
        std::fs::create_dir(&base).unwrap();
        let permissions = std::fs::metadata(&base).unwrap().permissions();
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o000)).unwrap();
        assert_eq!(
            std::fs::read_dir(&base).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        let mut app = selected_app(root.path(), &base, restored);
        let error = reject_without_copying(&mut app);
        copy_names_and_full_paths(&mut app, root.path());
        std::fs::set_permissions(&base, permissions).unwrap();
        assert!(error.contains("Permission denied"), "{error}");
    }
}

#[test]
fn base_removed_after_submission_is_checked_in_the_background_before_delivery() {
    let root = root();
    let base = root.path().join("base");
    std::fs::create_dir(&base).unwrap();
    let mut app = selected_app(root.path(), &base, true);
    let (ready, received) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let mut first = true;
    let mut runtime = Runtime::default();
    runtime
        .start_inspecting(
            copy_request(&mut app, "relative"),
            move |path| {
                if first {
                    first = false;
                    ready.send(()).unwrap();
                    blocked.recv_timeout(Duration::from_secs(10)).unwrap();
                }
                crate::fs::inspect_identity(path)
            },
            |_| panic!("invalid base must never reach clipboard provider"),
        )
        .unwrap();
    received.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(runtime.poll(&mut Vec::new()).is_none());
    std::fs::remove_dir(&base).unwrap();
    release.send(()).unwrap();
    let mut terminal = Vec::new();
    let error = wait(&mut runtime, &mut terminal).unwrap_err();
    assert!(error.contains("relative-path base"), "{error}");
    assert!(terminal.is_empty());
    assert_eq!(app.relative_path_base, base);
}

#[cfg(unix)]
#[test]
fn accessible_symlink_base_keeps_its_lexical_spelling() {
    let root = root();
    let physical = root.path().join("physical/deeper");
    std::fs::create_dir_all(&physical).unwrap();
    let base = root.path().join("alias");
    std::os::unix::fs::symlink(&physical, &base).unwrap();
    let mut app = selected_app(root.path(), &base, true);
    let text = copy_request(&mut app, "relative").prepare().unwrap();
    assert_eq!(text, Path::new("..").join(TEXT).to_str().unwrap());
    assert_eq!(app.relative_path_base, base);
}
