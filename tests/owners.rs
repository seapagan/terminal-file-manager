//! Owner presentation is view policy, independent of raw metadata identity.
use terminal_file_manager::settings::Settings;

#[test]
fn owner_representation_round_trips_in_defaults_and_named_profiles() {
    let settings =
        Settings::parse("owner_display = 'numeric'\n[profiles.Work]\nowner_display = 'names'")
            .unwrap();
    let encoded = toml::to_string(&settings).unwrap();
    assert!(encoded.contains("owner_display = \"numeric\""));
    assert!(Settings::parse(&encoded).is_ok());
}

use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::{sync::atomic::AtomicBool, time::Duration};
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs,
    input::{Action, Bindings, Controller},
    owner::OwnerDisplay,
    ui,
};

fn loaded() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("entry")).unwrap();
    let mut app = App::new(root.path().into());
    for id in 0..2 {
        app.complete_listing(
            id,
            1,
            Ok(fs::list(
                root.path(),
                &fs::IgnoreContext::isolated(),
                &AtomicBool::new(false),
            )
            .unwrap()),
        );
    }
    app.dual = true;
    let controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    (root, controller)
}

fn owner_cell(app: &App, id: usize, width: u16) -> (ui::Geometry, u16) {
    let geometry = ui::geometry(app, Rect::new(0, 0, width, 20));
    let area = geometry.panes[id].unwrap();
    let mut terminal = Terminal::new(TestBackend::new(width, 20)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let x = (area.x..area.right() - 5)
        .find(|x| {
            (*x..*x + 5)
                .map(|x| buffer[(x, 1)].symbol())
                .collect::<String>()
                == "Owner"
        })
        .unwrap();
    (geometry, x)
}

fn click(
    c: &mut Controller,
    geometry: &ui::Geometry,
    x: u16,
    y: u16,
) -> Vec<terminal_file_manager::app::Request> {
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }),
        geometry,
        Duration::from_millis(100),
    )
}

#[test]
fn only_entry_owner_cells_toggle_and_directory_double_click_still_enters() {
    let (_root, mut c) = loaded();
    for id in 0..2 {
        let (geometry, x) = owner_cell(&c.app, id, 300);
        click(&mut c, &geometry, x, 1);
        assert_eq!(c.app.panes[id].owner_display, OwnerDisplay::Names);
        click(&mut c, &geometry, x, 12);
        assert_eq!(c.app.panes[id].owner_display, OwnerDisplay::Names);
        assert!(click(&mut c, &geometry, x, 2).is_empty());
        assert_eq!(c.app.panes[id].owner_display, OwnerDisplay::Numeric);
        assert_eq!(c.app.active, id);
        assert_eq!(c.preferences.owner_display, OwnerDisplay::Numeric);
        assert!(click(&mut c, &geometry, x, 2).is_empty());
        assert_eq!(c.app.panes[id].owner_display, OwnerDisplay::Names);
        let name = geometry.panes[id].unwrap().x + 8;
        assert!(click(&mut c, &geometry, name, 2).is_empty());
        assert_eq!(click(&mut c, &geometry, name, 2).len(), 1);
    }
}

#[test]
fn view_menu_and_profiles_preserve_the_last_explicit_representation() {
    let (root, mut c) = loaded();
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    c.dispatch(Action::ViewOptions, &geometry);
    for code in std::iter::repeat_n(KeyCode::Down, 14).chain([KeyCode::Enter, KeyCode::Esc]) {
        c.handle(
            Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            &geometry,
            Duration::ZERO,
        );
    }
    assert_eq!(c.app.panes[0].owner_display, OwnerDisplay::Numeric);
    assert_eq!(c.app.panes[1].owner_display, OwnerDisplay::Names);
    c.dispatch(Action::FocusPane, &geometry);
    assert_eq!(c.preferences.owner_display, OwnerDisplay::Numeric);
    let view = terminal_file_manager::view::ViewSettings::capture(&c.app.panes[0]);
    view.apply(&mut c.app.panes[1]);
    assert_eq!(c.app.panes[1].owner_display, OwnerDisplay::Numeric);
    c.preferences.profiles.insert("Work".into(), view.clone());
    let store = terminal_file_manager::settings::Store {
        path: root.path().join("config.toml"),
        settings: c.preferences,
    };
    store.save().unwrap();
    let settings = terminal_file_manager::settings::Store::load(store.path)
        .unwrap()
        .settings;
    assert_eq!(settings.profiles["Work"], view);
    assert_eq!(settings.owner_display, OwnerDisplay::Numeric);
}

#[cfg(unix)]
#[test]
fn unix_snapshot_retains_ids_and_cached_names_remeasure_after_toggle() {
    use std::os::unix::fs::MetadataExt;
    let (root, mut c) = loaded();
    let meta = std::fs::symlink_metadata(root.path().join("entry")).unwrap();
    let pane = &mut c.app.panes[0];
    let owner = pane.entries[0].owner.as_mut().unwrap();
    assert_eq!((owner.uid, owner.gid), (meta.uid(), meta.gid()));
    owner.user = "界e\u{301}longaccountname".into();
    owner.group = "group".into();
    pane.refilter();
    let names_width = pane.measurements.owner;
    let numeric = format!("{}:{}", meta.uid(), meta.gid());
    std::fs::remove_dir(root.path().join("entry")).unwrap();
    let (geometry, _) = owner_cell(&c.app, 0, 300);
    c.dispatch(Action::OwnerDisplay, &geometry);
    assert_eq!(
        c.app.panes[0].entries[0].owner_display(OwnerDisplay::Numeric),
        numeric
    );
    assert!(c.app.panes[0].measurements.owner < names_width);
    let _ = owner_cell(&c.app, 0, 300);
}

#[cfg(windows)]
#[test]
fn windows_ownership_remains_unavailable_in_both_representations() {
    let (_root, c) = loaded();
    let entry = &c.app.panes[0].entries[0];
    assert!(entry.owner.is_none());
    for mode in [OwnerDisplay::Names, OwnerDisplay::Numeric] {
        assert_eq!(entry.owner_display(mode), "unavailable");
    }
}

#[test]
fn owner_hit_regions_follow_resizing_and_do_not_consume_marks_or_blank_space() {
    let (_root, mut c) = loaded();
    for id in 0..2 {
        c.app.panes[id].select_all();
    }
    for width in [220, 260, 300] {
        for percent in [40, 50, 60] {
            c.app.split_percent = percent;
            for id in 0..2 {
                let (geometry, x) = owner_cell(&c.app, id, width);
                let marks = c.app.panes[id].marks.clone();
                let previous = c.app.panes[id].owner_display;
                click(&mut c, &geometry, x, 2);
                assert_ne!(c.app.panes[id].owner_display, previous);
                assert_eq!(c.app.panes[id].marks, marks);
                let previous = c.app.panes[id].owner_display;
                let edge = geometry.panes[id].unwrap().right() - 1;
                click(&mut c, &geometry, edge, 2);
                assert_eq!(c.app.panes[id].owner_display, previous);
            }
        }
    }
}
