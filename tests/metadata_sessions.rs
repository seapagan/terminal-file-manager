//! Each open metadata surface owns temporary formatting and retains cached data.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::{Duration, SystemTime};
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs,
    input::{Action, Bindings, Controller},
    owner::{OwnerDisplay, Ownership},
    permissions::PermissionDisplay,
    settings::{Settings, Store},
    ui,
    view::ViewSettings,
};

fn loaded() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(root.path().into());
    app.now = SystemTime::UNIX_EPOCH + Duration::from_secs(172800);
    for id in 0..2 {
        let path = root.path().join(format!("entry{id}"));
        std::fs::write(&path, "data").unwrap();
        let mut entry = fs::inspect(&path);
        entry.mode = Some(0o104754);
        entry.modified = Some(app.now - Duration::from_secs(86400));
        entry.owner = Some(Ownership {
            uid: 12,
            gid: 34,
            user: "cached-user".into(),
            group: "cached-group".into(),
        });
        app.complete_listing(
            id,
            1,
            Ok(fs::Snapshot {
                path: root.path().into(),
                entries: vec![entry.clone()],
                errors: vec![],
            }),
        );
        let request = app.request_metadata(id).unwrap();
        assert!(app.complete_metadata(&request, entry, None));
        std::fs::remove_file(path).unwrap();
        app.panes[id].profile = Some("Detailed".into());
    }
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Settings::default(),
        ),
    )
}
fn action(c: &mut Controller, a: Action) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 180, 40));
    c.dispatch(a, &g);
}
fn click_value(c: &mut Controller, a: Action) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 180, 40));
    let cell = g
        .metadata_controls
        .iter()
        .find(|control| control.action == a)
        .unwrap()
        .area;
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: cell.x,
            row: cell.y,
            modifiers: KeyModifiers::NONE,
        }),
        &g,
        Duration::ZERO,
    );
}
fn text(c: &Controller) -> String {
    let mut t = Terminal::new(TestBackend::new(180, 40)).unwrap();
    t.draw(|f| {
        ui::render(f, &c.app, &Theme::new(ColorLevel::Ansi16));
    })
    .unwrap();
    t.backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}
#[test]
fn both_panes_and_surfaces_toggle_locally_without_saving_or_changing_profiles() {
    for id in 0..2 {
        for floating in [false, true] {
            let (root, mut c) = loaded();
            c.app.active = id;
            action(
                &mut c,
                if floating {
                    Action::Metadata
                } else {
                    Action::ToggleSidebar
                },
            );
            c.preferences_changed = false;
            let badges = c.app.panes.each_ref().map(|p| p.profile_label());
            let views = c.app.panes.each_ref().map(ViewSettings::capture);
            let before = toml::to_string(&c.preferences).unwrap();
            let store = Store {
                path: root.path().join("config.toml"),
                settings: c.preferences.clone(),
            };
            store.save().unwrap();
            for a in [
                Action::Permissions,
                Action::FuzzyTimestamps,
                Action::OwnerDisplay,
            ] {
                click_value(&mut c, a);
            }
            assert_eq!(c.app.panes.each_ref().map(ViewSettings::capture), views);
            assert_eq!(c.app.panes.each_ref().map(|p| p.profile_label()), badges);
            assert_eq!(toml::to_string(&c.preferences).unwrap(), before);
            assert!(!c.preferences_changed);
            assert_eq!(std::fs::read_to_string(&store.path).unwrap(), before);
            let rendered = text(&c);
            assert!(
                rendered.contains("- 4754")
                    && rendered.contains("yesterday")
                    && rendered.contains("12:34")
            );
            assert!(c.app.request_metadata(id).is_none());
            reopen_defaults(&mut c, floating);
        }
    }
}
fn reopen_defaults(c: &mut Controller, floating: bool) {
    action(
        c,
        if floating {
            Action::Cancel
        } else {
            Action::ToggleSidebar
        },
    );
    action(
        c,
        if floating {
            Action::Metadata
        } else {
            Action::ToggleSidebar
        },
    );
    let format = c.app.metadata_format(floating);
    assert_eq!(format.permissions, PermissionDisplay::Symbolic);
    assert!(!format.fuzzy_timestamps);
    assert_eq!(format.owner_display, OwnerDisplay::Names);
}

#[test]
fn popup_and_sidebar_invocations_are_independent_and_entry_changes_retain_choices() {
    let (_root, mut c) = loaded();
    action(&mut c, Action::ToggleSidebar);
    click_value(&mut c, Action::Permissions);
    let first = c.app.details[0].entry.clone().unwrap();
    let next = c.app.details[1].entry.clone().unwrap();
    c.app.complete_listing(
        0,
        1,
        Ok(fs::Snapshot {
            path: c.app.panes[0].path.clone(),
            entries: vec![first, next.clone()],
            errors: vec![],
        }),
    );
    action(&mut c, Action::Down);
    let request = c.app.request_metadata(0).unwrap();
    assert_eq!(request.path, next.path);
    assert!(c.app.complete_metadata(&request, next, None));
    assert!(text(&c).contains("- 4754"));
    action(&mut c, Action::Metadata);
    assert_eq!(
        c.app.metadata_format(true).permissions,
        PermissionDisplay::Symbolic
    );
    click_value(&mut c, Action::OwnerDisplay);
    action(&mut c, Action::Cancel);
    assert_eq!(
        c.app.metadata_format(false).permissions,
        PermissionDisplay::Octal
    );
    assert_eq!(
        c.app.metadata_format(false).owner_display,
        OwnerDisplay::Names
    );
    action(&mut c, Action::Metadata);
    assert_eq!(
        c.app.metadata_format(true).owner_display,
        OwnerDisplay::Names
    );
}
#[test]
fn focus_and_layout_hiding_restart_metadata_from_the_new_pane_defaults() {
    let (_root, mut c) = loaded();
    c.app.panes[1].permissions = PermissionDisplay::Octal;
    c.app.panes[1].fuzzy_timestamps = true;
    c.app.panes[1].owner_display = OwnerDisplay::Numeric;
    action(&mut c, Action::ToggleSidebar);
    click_value(&mut c, Action::Permissions);
    let g = ui::geometry(&c.app, Rect::new(0, 0, 180, 40));
    c.handle(Event::Resize(40, 12), &g, Duration::ZERO);
    assert!(c.app.metadata_formats[0].is_none());
    c.handle(Event::Resize(180, 40), &g, Duration::ZERO);
    assert_eq!(
        c.app.metadata_format(false).permissions,
        PermissionDisplay::Symbolic
    );
    action(&mut c, Action::TogglePanes);
    action(&mut c, Action::Metadata);
    click_value(&mut c, Action::Permissions);
    action(&mut c, Action::FocusPane);
    let format = c.app.metadata_format(true);
    assert_eq!(format.permissions, PermissionDisplay::Octal);
    assert!(format.fuzzy_timestamps);
    assert_eq!(format.owner_display, OwnerDisplay::Numeric);
    assert!(c.app.metadata_formats[0].is_none());
    action(&mut c, Action::FocusPane);
    assert_eq!(
        c.app.metadata_format(true).permissions,
        PermissionDisplay::Symbolic
    );
}
