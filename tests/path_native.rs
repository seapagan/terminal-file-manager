//! Run native path spelling/identity cases on the platform that owns them.
#[cfg(unix)]
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
#[cfg(unix)]
use ratatui::layout::Rect;
use std::path::{Path, PathBuf};
use terminal_file_manager::{app::App, navigation::path_segments, ui};
#[cfg(unix)]
use terminal_file_manager::{
    input::{Bindings, Controller},
    settings::Settings,
};

fn assert_segments(path: &Path) {
    let segments = path_segments(path);
    assert_eq!(
        segments.iter().map(|s| s.text.as_str()).collect::<String>(),
        ui::clean(&path.to_string_lossy())
    );
    for segment in segments {
        assert!(
            path.ancestors()
                .any(|ancestor| ancestor.as_os_str().as_encoded_bytes()
                    == segment.path.as_os_str().as_encoded_bytes())
        );
    }
}

#[test]
fn native_spelling_roots_relative_paths_and_separators_retain_exact_targets() {
    for path in [
        PathBuf::from("."),
        PathBuf::from(".."),
        PathBuf::from("alpha/./beta/../leaf/"),
        std::env::temp_dir(),
    ] {
        assert_segments(&path);
        for ancestor in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
            assert_segments(ancestor);
        }
    }
}

#[cfg(unix)]
#[test]
fn symlink_ancestors_navigate_lexically_and_failed_navigation_preserves_history() {
    let root = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    std::fs::create_dir(target.path().join("child")).unwrap();
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(target.path(), &alias).unwrap();
    let path = alias.join("child");
    assert_segments(&path);
    let mut c = Controller::new(
        App::new(path.clone()),
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    let g = ui::geometry(&c.app, Rect::new(0, 0, 240, 24));
    let control = g
        .path_controls
        .iter()
        .find(|c| c.target.as_deref() == Some(alias.as_path()))
        .unwrap();
    let request = c
        .handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: control.area.x,
                row: control.area.y,
                modifiers: KeyModifiers::NONE,
            }),
            &g,
            Default::default(),
        )
        .remove(0);
    assert_eq!(request.path, alias);
    c.app
        .complete_listing(0, request.generation, Err("denied".into()));
    assert_eq!(c.app.panes[0].path, path);
    assert!(c.app.history(false).is_none());
}

// APFS cannot create this fixture; lexical byte mapping itself remains portable Unix.
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_display_collisions_keep_distinct_native_ancestor_targets() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join(OsString::from_vec(b"bad\xfe".to_vec()));
    let second = root.path().join(OsString::from_vec(b"bad\xff".to_vec()));
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    assert_eq!(first.to_string_lossy(), second.to_string_lossy());
    for ancestor in [first, second] {
        let path = ancestor.join("leaf");
        assert_segments(&path);
        let segments = path_segments(&path);
        assert_eq!(
            segments[segments.len() - 2]
                .path
                .as_os_str()
                .as_encoded_bytes(),
            ancestor.as_os_str().as_encoded_bytes()
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_drive_unc_verbatim_and_non_utf16_paths_keep_native_ancestors() {
    use std::{
        ffi::OsString,
        os::windows::ffi::{OsStrExt, OsStringExt},
    };
    for raw in [
        r"C:\",
        r"C:\alpha\beta",
        r"C:/alpha/beta/",
        r"\\server\share\",
        r"\\server\share\alpha\beta",
        r"\\?\C:\alpha\beta",
        r"\\?\UNC\server\share\alpha",
    ] {
        let path = Path::new(raw);
        assert_segments(path);
        let app = App::new(path.into());
        assert_eq!(app.panes[0].has_parent(), path.parent().is_some());
    }
    let mut units = r"C:\bad".encode_utf16().collect::<Vec<_>>();
    units.push(0xd800);
    let ancestor = PathBuf::from(OsString::from_wide(&units));
    let path = ancestor.join("leaf");
    assert_segments(&path);
    let segments = path_segments(&path);
    assert_eq!(
        segments[segments.len() - 2]
            .path
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>(),
        units
    );
}
