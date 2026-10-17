//! Native navigation data never stores view profiles, marks or history.
use terminal_file_manager::{
    app::App,
    session::{NavigationSession, SessionStore},
    settings::Settings,
};

#[test]
fn session_storage_round_trips_layout_and_both_native_locations() {
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(root.path().join("left"));
    app.panes[1].path = root.path().join("right");
    app.active = 1;
    app.dual = true;
    app.maximized = true;
    app.split_percent = 65;
    let store = SessionStore {
        path: root.path().join("session.toml"),
    };
    store.save(&app).unwrap();
    let session = store.load().unwrap();
    let restored = session.restore(&Settings::default()).unwrap();
    assert_eq!(restored.panes[0].path, app.panes[0].path);
    assert_eq!(restored.panes[1].path, app.panes[1].path);
    assert_eq!(restored.active, 1);
    assert!(restored.dual && restored.maximized);
    assert_eq!(restored.split_percent, 65);
    assert!(
        restored
            .panes
            .iter()
            .all(|pane| pane.marks.is_empty() && pane.filter.is_empty())
    );
    let text = std::fs::read_to_string(&store.path).unwrap();
    assert!(!text.contains("history") && !text.contains("profiles"));
    assert!(NavigationSession::parse(&text.replace("version = 1", "version = 2")).is_err());
}

#[cfg(unix)]
#[test]
fn unix_session_paths_retain_non_utf8_bytes() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let path = root
        .path()
        .join(std::ffi::OsString::from_vec(vec![b'x', 0xff, 0xfe]));
    let app = App::new(path.clone());
    let store = SessionStore {
        path: root.path().join("session.toml"),
    };
    store.save(&app).unwrap();
    assert_eq!(
        store.load().unwrap().locations().unwrap(),
        [path.clone(), path]
    );
}

#[cfg(windows)]
#[test]
fn windows_session_paths_retain_unpaired_utf16_units() {
    use std::os::windows::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let path = root
        .path()
        .join(std::ffi::OsString::from_wide(&[b'x' as u16, 0xd800]));
    let app = App::new(path.clone());
    let store = SessionStore {
        path: root.path().join("session.toml"),
    };
    store.save(&app).unwrap();
    assert_eq!(
        store.load().unwrap().locations().unwrap(),
        [path.clone(), path]
    );
}

#[test]
fn session_rejects_invalid_schema_and_preserves_existing_destination() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore {
        path: root.path().join("session.toml"),
    };
    store.save(&App::new(root.path().to_owned())).unwrap();
    let original = std::fs::read_to_string(&store.path).unwrap();
    for invalid in [
        original.replace("active = 0", "active = 2"),
        original.replace("split_percent = 50", "split_percent = 1"),
        format!("{original}\nunknown = true\n"),
    ] {
        assert!(NavigationSession::parse(&invalid).is_err());
    }
    assert!(store.save(&App::new("relative".into())).is_err());
    assert_eq!(std::fs::read_to_string(&store.path).unwrap(), original);
    let directory = SessionStore {
        path: root.path().to_owned(),
    };
    assert!(directory.save(&App::new(root.path().to_owned())).is_err());
    assert!(directory.load().is_err());
}

#[cfg(unix)]
#[test]
fn session_refuses_linked_storage_without_touching_its_target() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    std::fs::write(&target, b"original").unwrap();
    let store = SessionStore {
        path: root.path().join("session.toml"),
    };
    std::os::unix::fs::symlink(&target, &store.path).unwrap();
    assert!(store.save(&App::new(root.path().to_owned())).is_err());
    assert!(store.load().is_err());
    assert_eq!(std::fs::read(target).unwrap(), b"original");
}
