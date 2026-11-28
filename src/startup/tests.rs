use super::*;
use crate::{
    fs,
    input::{Action, Bindings, Controller},
    settings::Settings,
    ui,
};
use std::sync::atomic::AtomicBool;

fn install(app: &mut crate::app::App, request: &crate::app::Request) {
    let snapshot = fs::list(
        request.local_path(),
        &fs::IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(app.complete_listing(request.pane, request.generation, Ok(snapshot)));
}

fn assert_initial_panes(app: &crate::app::App, paths: [&std::path::Path; 2]) {
    for (pane, path) in app.panes.iter().zip(paths) {
        assert_eq!(*pane.local_path(), path);
        assert_eq!(pane.cursor.as_deref(), path.parent(), "focus in {path:?}");
        assert!(pane.parent_focused(), "parent row in {path:?}");
        assert_eq!(pane.row_index(), Some(0));
    }
}

#[test]
fn session_startup_normalizes_locations_before_initializing_focus() {
    let root = tempfile::tempdir().unwrap();
    let left = root.path().join("first/left");
    let right = root.path().join("second/right");
    let saved_left = left.join("child/..");
    let saved_right = right.join("child/..");
    for path in [&left, &right] {
        std::fs::create_dir_all(path.join("child")).unwrap();
    }
    let mut saved = crate::app::App::with_locations([saved_left, saved_right]);
    saved.dual = true;
    let session = crate::session::NavigationSession::capture(&saved);
    let app = session
        .restore_with(&Settings::default(), |path| validate_path(&path))
        .unwrap();
    assert_initial_panes(&app, [&left, &right]);
    assert!(app.dual);
    assert_eq!(app.relative_path_base, saved.relative_path_base);
}

#[test]
fn startup_highlights_parent_in_both_panes_for_explicit_paths() {
    let root = tempfile::tempdir().unwrap();
    let left = root.path().join("first/left");
    let right = root.path().join("second/right");
    for path in [&left, &right] {
        std::fs::create_dir_all(path).unwrap();
        std::fs::write(path.join("visible"), b"fixture").unwrap();
    }
    for (paths, expected) in [
        (vec![left.as_path()], [&left, &left]),
        (vec![left.as_path(), right.as_path()], [&left, &right]),
    ] {
        let startup = Startup {
            paths: paths.into_iter().map(PathBuf::from).collect(),
            ..Startup::default()
        };
        let mut app = startup.prepare(&Settings::default()).unwrap();
        let expected = expected.map(PathBuf::as_path);
        assert_initial_panes(&app, expected);
        assert_eq!(app.dual, startup.paths.len() == 2);
        for (pane, path) in expected.into_iter().enumerate() {
            let request = app.request_listing(pane, path.into());
            install(&mut app, &request);
        }
        assert_initial_panes(&app, expected);
    }
}

#[test]
fn startup_parent_and_history_use_normalized_explicit_locations() {
    let root = tempfile::tempdir().unwrap();
    let start = root.path().join("start");
    std::fs::create_dir_all(start.join("child")).unwrap();
    let startup = Startup {
        paths: vec![start.join("child/..")],
        ..Startup::default()
    };
    let mut app = startup.prepare(&Settings::default()).unwrap();
    assert_initial_panes(&app, [&start, &start]);
    assert_eq!(app.relative_path_base, start);
    let initial = app.request_listing(0, app.panes[0].local_path().clone());
    install(&mut app, &initial);
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    let parent = controller
        .dispatch(Action::Parent, &geometry)
        .pop()
        .unwrap();
    assert_eq!(parent.local_path(), root.path());
    install(&mut controller.app, &parent);
    let back = controller.app.history(false).unwrap();
    assert_eq!(back.local_path(), &start);
    install(&mut controller.app, &back);
    let forward = controller.app.history(true).unwrap();
    assert_eq!(forward.local_path(), root.path());
}

#[test]
fn root_parent_components_stay_at_the_native_root_and_invalid_paths_do_not_fall_back() {
    let root = tempfile::tempdir().unwrap();
    let native_root = root.path().ancestors().last().unwrap();
    assert_eq!(validate_path(&native_root.join("..")).unwrap(), native_root);
    assert!(validate_path(&root.path().join("missing")).is_err());
    std::fs::write(root.path().join("file"), b"data").unwrap();
    assert!(validate_path(&root.path().join("file")).is_err());
    #[cfg(unix)]
    {
        assert!(validate_path(&root.path().join("missing/..")).is_err());
        assert!(validate_path(&root.path().join("file/..")).is_err());
    }
}

#[cfg(unix)]
#[test]
fn direct_links_keep_logical_navigation_but_explicit_link_parent_keeps_os_target() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("physical/deep")).unwrap();
    let link = root.path().join("logical");
    std::os::unix::fs::symlink("physical/deep", &link).unwrap();
    assert_eq!(validate_path(&link).unwrap(), link);
    assert_eq!(validate_path(&link).unwrap().parent(), Some(root.path()));
    let normalized = validate_path(&link.join("..")).unwrap();
    assert_eq!(
        normalized,
        std::fs::canonicalize(root.path().join("physical")).unwrap()
    );
    assert_eq!(
        std::fs::canonicalize(normalized).unwrap(),
        std::fs::canonicalize(link.join("..")).unwrap()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn normalization_preserves_non_utf8_native_components() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempfile::tempdir().unwrap();
    let native = root.path().join(OsString::from_vec(vec![b'd', 0xff]));
    std::fs::create_dir_all(native.join("child")).unwrap();
    assert_eq!(validate_path(&native.join("child/..")).unwrap(), native);
}

#[cfg(windows)]
#[test]
fn windows_drive_unc_and_verbatim_roots_keep_their_prefixes() {
    for (input, expected) in [
        (r"C:\..", r"C:\"),
        (r"\\server\share\..", r"\\server\share\"),
        (r"\\?\C:\..", r"\\?\C:\"),
        (r"\\?\UNC\server\share\..", r"\\?\UNC\server\share\"),
    ] {
        assert_eq!(
            normalize_path(std::path::Path::new(input)).unwrap(),
            std::path::Path::new(expected)
        );
    }
}

#[test]
fn startup_base_uses_first_explicit_directory_or_process_cwd() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    for paths in [vec![first.clone()], vec![first.clone(), second]] {
        let app = Startup {
            paths,
            ..Startup::default()
        }
        .prepare(&Settings::default())
        .unwrap();
        assert_eq!(app.relative_path_base, first);
    }
    let relative = Startup {
        paths: vec![".".into()],
        ..Startup::default()
    }
    .prepare(&Settings::default())
    .unwrap();
    assert_eq!(
        relative.relative_path_base,
        std::env::current_dir().unwrap()
    );
    let app = Startup::default().prepare(&Settings::default()).unwrap();
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(app.relative_path_base, cwd);
    assert_initial_panes(&app, [&cwd, &cwd]);
    assert_initial_panes(&relative, [&cwd, &cwd]);
}

#[test]
fn too_many_startup_paths_fail_before_validating_the_first_path() {
    let root = tempfile::tempdir().unwrap();
    let startup = Startup {
        paths: vec![root.path().join("missing"); 3],
        ..Startup::default()
    };
    assert_eq!(
        startup.prepare(&Settings::default()).err().unwrap(),
        "at most two directory paths are supported"
    );
}
