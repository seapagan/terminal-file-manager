//! Parent focus uses the original lexical entry, including direct-start links.
use std::{path::Path, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::App,
    app::Request,
    fs::{self, IgnoreContext},
};

fn complete(app: &mut App, request: Request) {
    let listing = fs::list(
        &request.path,
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(app.complete_listing(0, request.generation, Ok(listing)));
}

fn loaded(path: &Path) -> App {
    let mut app = App::new(path.into());
    complete(
        &mut app,
        Request {
            pane: 0,
            generation: 1,
            path: path.into(),
            kind: terminal_file_manager::app::RequestKind::Listing,
        },
    );
    app
}

#[cfg(unix)]
fn link(target: &Path, alias: &Path) {
    std::os::unix::fs::symlink(target, alias).unwrap();
}

#[cfg(windows)]
fn link(target: &Path, alias: &Path) {
    assert!(
        std::process::Command::new("cmd")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(alias)
            .arg(target)
            .status()
            .unwrap()
            .success()
    );
}

fn fixture() -> (tempfile::TempDir, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("aaa")).unwrap();
    link(target.path(), &root.path().join("link"));
    (root, target)
}

#[test]
fn direct_start_and_saved_visits_restore_the_link_entry() {
    for history in ["direct", "back", "forward", "unrelated"] {
        let (root, _target) = fixture();
        let alias = root.path().join("link");
        let mut app = loaded(if history == "back" || history == "unrelated" {
            root.path()
        } else {
            &alias
        });
        if history == "back" || history == "unrelated" {
            let request = app.request_listing(
                0,
                if history == "back" {
                    alias.clone()
                } else {
                    root.path().join("aaa")
                },
            );
            complete(&mut app, request);
            if history == "unrelated" {
                let request = app.request_listing(0, alias.clone());
                complete(&mut app, request);
            }
        } else if history == "forward" {
            let request = app.parent().unwrap();
            complete(&mut app, request);
            let request = app.history(false).unwrap();
            complete(&mut app, request);
        }
        let request = app.parent().unwrap();
        complete(&mut app, request);
        assert_eq!(app.panes[0].cursor, Some(alias), "{history}");
        assert!(app.panes[0].marks.is_empty());
    }
}

#[test]
fn direct_start_link_replacement_or_removal_cannot_restore_focus() {
    for replace in [false, true] {
        let (root, target) = fixture();
        let alias = root.path().join("link");
        let mut app = loaded(&alias);
        std::fs::rename(&alias, root.path().join("old-link")).unwrap();
        if replace {
            link(target.path(), &alias);
        }
        // A same-target refresh must not adopt the replacement link identity.
        let request = app.request_refresh(0).unwrap();
        app.complete_listing(
            0,
            request.generation,
            fs::list(&alias, &IgnoreContext::isolated(), &AtomicBool::new(false))
                .map_err(|e| e.to_string()),
        );
        let request = app.parent().unwrap();
        complete(&mut app, request);
        assert_eq!(app.panes[0].cursor, Some(root.path().join("aaa")));
    }
}

#[test]
fn direct_start_link_obeys_visibility_and_missing_identity() {
    for exclusion in ["hidden", "absent", "identity", "filter"] {
        let (root, _target) = fixture();
        let alias = root.path().join("link");
        let mut app = loaded(&alias);
        let request = app.parent().unwrap();
        let mut listing = fs::list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap();
        match exclusion {
            "hidden" => {
                listing
                    .entries
                    .iter_mut()
                    .find(|e| e.path == alias)
                    .unwrap()
                    .hidden = true
            }
            "absent" => listing.entries.retain(|e| e.path != alias),
            "identity" => {
                listing
                    .entries
                    .iter_mut()
                    .find(|e| e.path == alias)
                    .unwrap()
                    .identity = None
            }
            _ => app.panes[0].set_filter("aaa".into()),
        }
        assert!(app.complete_listing(0, request.generation, Ok(listing)));
        assert_eq!(app.panes[0].cursor, Some(root.path().join("aaa")));
    }
}

#[test]
fn missing_original_lexical_identity_uses_safe_parent_fallback() {
    let (root, _target) = fixture();
    let alias = root.path().join("link");
    let mut app = App::new(alias.clone());
    let mut listing =
        fs::list(&alias, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap();
    listing.entry_identity = None;
    app.complete_listing(0, 1, Ok(listing));
    let request = app.parent().unwrap();
    complete(&mut app, request);
    assert_eq!(app.panes[0].cursor, Some(root.path().join("aaa")));
}

#[cfg(windows)]
#[test]
fn windows_directory_symlink_restores_focus_where_creation_is_supported() {
    let root = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let alias = root.path().join("symlink");
    std::fs::create_dir(root.path().join("aaa")).unwrap();
    if let Err(error) = std::os::windows::fs::symlink_dir(target.path(), &alias) {
        assert_eq!(
            error.raw_os_error(),
            Some(1314),
            "unexpected symlink failure"
        );
        eprintln!(
            "Directory symlink creation unavailable: {error}; junction cases remain mandatory"
        );
        return;
    }
    let mut app = loaded(&alias);
    let request = app.parent().unwrap();
    complete(&mut app, request);
    assert_eq!(app.panes[0].cursor, Some(alias));
}
