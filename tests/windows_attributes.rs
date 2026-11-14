//! Native Windows RHSA snapshots, including rendering after the source disappears.
#![cfg(windows)]
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use std::{fs, os::windows::fs::MetadataExt, process::Command, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs::{self as local, IgnoreContext},
    permissions::{PermissionDisplay, attributes},
    ui,
};

fn set_attributes(path: &std::path::Path, symbolic: &str) {
    let mut command = Command::new("attrib");
    for letter in ['R', 'H', 'S', 'A'] {
        command.arg(format!(
            "{}{letter}",
            if symbolic.contains(letter) { '+' } else { '-' }
        ));
    }
    let output = command.arg(path).output().unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn native_rhsa_positions_preserve_full_hex_and_use_cached_metadata() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("attributes");
    fs::write(&path, b"data").unwrap();
    for expected in [
        "----", "R---", "-H--", "RH--", "--S-", "R-S-", "-HS-", "RHS-", "---A", "R--A", "-H-A",
        "RH-A", "--SA", "R-SA", "-HSA", "RHSA",
    ] {
        set_attributes(&path, expected);
        let entry = local::inspect(&path);
        let native = fs::symlink_metadata(&path).unwrap().file_attributes();
        set_attributes(&path, "----");
        assert_eq!(attributes(&entry, PermissionDisplay::Symbolic), expected);
        assert_eq!(
            attributes(&entry, PermissionDisplay::Octal),
            format!("0x{native:08x}")
        );
    }
    set_attributes(&path, "-HSA");
    let cached = local::inspect(&path);
    set_attributes(&path, "----");
    fs::remove_file(&path).unwrap();
    assert_eq!(attributes(&cached, PermissionDisplay::Symbolic), "-HSA");
}

fn draw(app: &App) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(160, 30)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn text(buffer: &Buffer) -> String {
    buffer.content().iter().map(|cell| cell.symbol()).collect()
}

fn cached_directory_app() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("directory");
    fs::create_dir(&path).unwrap();
    set_attributes(&path, "R-SA");
    let snapshot = local::list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let entry = snapshot.entries[0].clone();
    set_attributes(&path, "----");
    fs::remove_dir(&path).unwrap();
    let mut app = App::new(root.path().into());
    app.complete_listing(0, 1, Ok(snapshot));
    app.panes[0].columns.attributes = true;
    app.details[0].entry = Some(entry.clone());
    app.details[0].path = Some(path);
    (root, app)
}

#[test]
fn native_attributes_render_in_listing_sidebar_and_popup_in_both_formats() {
    let (_root, mut app) = cached_directory_app();
    let native = app.details[0].entry.as_ref().unwrap().attributes.clone();
    for (display, expected) in [
        (PermissionDisplay::Symbolic, "R-SA".to_owned()),
        (PermissionDisplay::Octal, native),
    ] {
        app.panes[0].permissions = display;
        app.sidebar = false;
        app.popup = false;
        assert!(text(&draw(&app)).contains(&expected));
        for floating in [false, true] {
            app.sidebar = !floating;
            app.panes[0].sidebar = !floating;
            app.popup = floating;
            let buffer = draw(&app);
            let geometry = ui::geometry(&app, buffer.area);
            let value: String = geometry
                .metadata_controls
                .iter()
                .filter(|control| {
                    control.action == terminal_file_manager::input::Action::Permissions
                })
                .flat_map(|control| {
                    (control.area.x..control.area.right())
                        .map(|x| buffer[(x, control.area.y)].symbol())
                })
                .collect();
            assert_eq!(value, expected);
        }
    }
}

#[test]
fn windows_toggle_labels_native_hex_and_preserves_the_octal_setting() {
    use terminal_file_manager::input::{Action, Bindings, Controller};
    let root = tempfile::tempdir().unwrap();
    let mut controller = Controller::new(
        App::new(root.path().into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 160, 30));
    controller.dispatch(Action::Permissions, &geometry);
    assert_eq!(
        controller.app.panes[0].permissions,
        PermissionDisplay::Octal
    );
    let saved = toml::to_string(&controller.preferences).unwrap();
    assert!(saved.contains("permissions = \"octal\""));
    controller.app.view_menu = Some(2);
    assert!(text(&draw(&controller.app)).contains("Native hex"));
    controller.dispatch(Action::Permissions, &geometry);
    assert_eq!(
        controller.app.panes[0].permissions,
        PermissionDisplay::Symbolic
    );
}
