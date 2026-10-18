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
        &request.path,
        &fs::IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(app.complete_listing(request.pane, request.generation, Ok(snapshot)));
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
    assert_eq!(app.panes[0].path, start);
    assert_eq!(app.panes[1].path, start);
    let initial = app.request_listing(0, app.panes[0].path.clone());
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
    assert_eq!(parent.path, root.path());
    install(&mut controller.app, &parent);
    let back = controller.app.history(false).unwrap();
    assert_eq!(back.path, start);
    install(&mut controller.app, &back);
    let forward = controller.app.history(true).unwrap();
    assert_eq!(forward.path, root.path());
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
