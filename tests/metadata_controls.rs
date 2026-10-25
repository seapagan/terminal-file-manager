//! Metadata values share listing styles while keeping formatting local to each open view.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    fs,
    input::{Bindings, Controller},
    permissions::PermissionDisplay,
    ui,
};

fn loaded() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("selected");
    std::fs::write(&path, "data").unwrap();
    let mut app = App::new(root.path().into());
    let mut entry = fs::inspect(&path);
    entry.mode = Some(0o104754);
    app.details[0].entry = Some(entry);
    app.details[0].path = Some(path);
    app.sidebar = true;
    app.panes[0].sidebar = true;
    app.panes[0].profile = Some("Detailed".into());
    let c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    (root, c)
}

fn draw(app: &App, width: u16, height: u16, level: ColorLevel) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(level));
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn find(buffer: &Buffer, text: &str) -> (u16, u16) {
    for y in 0..buffer.area.height {
        let row: String = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect();
        if let Some(offset) = row.find(text) {
            return (row[..offset].chars().count() as u16, y);
        }
    }
    panic!("missing {text}");
}

fn click(c: &mut Controller, x: u16, y: u16, geometry: &ui::Geometry) {
    assert!(
        c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE,
            }),
            geometry,
            std::time::Duration::ZERO
        )
        .is_empty()
    );
}

fn visible_value(buffer: &Buffer, geometry: &ui::Geometry, text: &str) -> (u16, u16) {
    let control = geometry
        .metadata_controls
        .iter()
        .enumerate()
        .find_map(|(index, control)| {
            let mut value = String::new();
            for run in &geometry.metadata_controls[index..] {
                if run.action != control.action || value.len() >= text.len() {
                    break;
                }
                value.extend(
                    (run.area.x..run.area.right()).map(|x| buffer[(x, run.area.y)].symbol()),
                );
            }
            (value == text).then_some(control)
        })
        .expect("value runs match visible buffer content");
    (control.area.x, control.area.y)
}

#[test]
fn metadata_attributes_keep_symbolic_and_octal_character_styles_at_every_depth() {
    let (_root, mut c) = loaded();
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let theme = Theme::new(level);
        for floating in [false, true] {
            c.app.popup = floating;
            for format in [PermissionDisplay::Symbolic, PermissionDisplay::Octal] {
                c.app.panes[0].permissions = format;
                let entry = c.app.details[0].entry.as_ref().unwrap();
                let value = terminal_file_manager::permissions::attributes(entry, format);
                let buffer = draw(&c.app, 120, 32, level);
                let geometry = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
                let (x, y) = visible_value(&buffer, &geometry, &value);
                for (i, ch) in value.chars().enumerate() {
                    let style = if i == 0 {
                        theme.entry_style(entry)
                    } else if format == PermissionDisplay::Symbolic {
                        theme.permission_style(ch)
                    } else {
                        theme.style(Role::Warning)
                    };
                    let cell = &buffer[(x + i as u16, y)];
                    assert_eq!(cell.fg, style.fg.unwrap_or_default(), "{level:?} {ch}");
                    assert_eq!(cell.modifier, style.add_modifier, "{level:?} {ch}");
                }
            }
        }
    }
}

#[test]
fn attribute_value_click_changes_only_the_open_metadata_view() {
    for floating in [false, true] {
        let (_root, mut c) = loaded();
        c.app.popup = floating;
        let buffer = draw(&c.app, 120, 32, ColorLevel::NoColor);
        let geometry = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
        let (x, y) = visible_value(&buffer, &geometry, "-rwsr-xr--");
        click(&mut c, x - 2, y, &geometry);
        assert_eq!(c.app.panes[0].permissions, PermissionDisplay::Symbolic);
        assert!(!c.preferences_changed);
        click(&mut c, x + 1, y, &geometry);
        assert_eq!(c.app.panes[0].permissions, PermissionDisplay::Symbolic);
        assert_eq!(c.app.panes[1].permissions, PermissionDisplay::Symbolic);
        assert_eq!(c.preferences.permissions, PermissionDisplay::Symbolic);
        assert_eq!(
            c.app.metadata_format(floating).permissions,
            PermissionDisplay::Octal
        );
        assert!(!c.preferences_changed && !c.app.panes[0].profile_modified);
        let restored = terminal_file_manager::settings::Settings::parse(
            &toml::to_string(&c.preferences).unwrap(),
        )
        .unwrap();
        assert_eq!(restored.permissions, PermissionDisplay::Symbolic);
        let buffer = draw(&c.app, 120, 32, ColorLevel::NoColor);
        let geometry = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
        let (x, y) = visible_value(&buffer, &geometry, "- 4754");
        click(&mut c, x, y, &geometry);
        assert_eq!(c.app.panes[0].permissions, PermissionDisplay::Symbolic);
    }
}

fn show_attributes(c: &mut Controller, bounds: Rect, floating: bool) -> u16 {
    let geometry = ui::geometry(&c.app, bounds);
    let limit = if floating {
        geometry.overlay_scroll_limit
    } else {
        geometry.metadata_scroll_limit
    };
    (0..=limit)
        .find(|scroll| {
            if floating {
                c.app.overlay_scroll = *scroll;
            } else {
                c.app.metadata_scroll = *scroll;
            }
            let controls: Vec<_> = ui::geometry(&c.app, bounds)
                .metadata_controls
                .into_iter()
                .filter(|control| {
                    control.action == terminal_file_manager::input::Action::Permissions
                })
                .collect();
            let buffer = draw(&c.app, bounds.width, bounds.height, ColorLevel::Ansi16);
            let text: String = controls
                .iter()
                .flat_map(|control| {
                    (control.area.x..control.area.right())
                        .map(|x| buffer[(x, control.area.y)].symbol())
                })
                .collect();
            text == "-rwsr-xr--"
        })
        .expect("entire attribute value visible after scroll")
}

#[test]
fn wrapped_scrolled_values_match_click_regions_and_exclude_padding_and_overlays() {
    for (floating, width) in [(true, 24), (true, 40), (false, 120)] {
        let (_root, mut c) = loaded();
        c.app.popup = floating;
        let bounds = Rect::new(0, 0, width, 12);
        let scroll = show_attributes(&mut c, bounds, floating);
        assert!(scroll > 0);
        let mut geometry = ui::geometry(&c.app, bounds);
        geometry
            .metadata_controls
            .retain(|control| control.action == terminal_file_manager::input::Action::Permissions);
        let mut shifted = ui::geometry(&c.app, Rect::new(5, 7, width, 12));
        shifted
            .metadata_controls
            .retain(|control| control.action == terminal_file_manager::input::Action::Permissions);
        assert_eq!(
            geometry.metadata_controls.len(),
            shifted.metadata_controls.len()
        );
        for (control, translated) in geometry
            .metadata_controls
            .iter()
            .zip(&shifted.metadata_controls)
        {
            assert_eq!(translated.area.x, control.area.x + 5);
            assert_eq!(translated.area.y, control.area.y + 7);
            click(&mut c, control.area.x - 1, control.area.y, &geometry);
            click(&mut c, control.area.right(), control.area.y, &geometry);
            assert_eq!(c.app.panes[0].permissions, PermissionDisplay::Symbolic);
            click(&mut c, control.area.x, control.area.y, &geometry);
            assert_eq!(c.app.panes[0].permissions, PermissionDisplay::Symbolic);
            assert_eq!(
                c.app.metadata_format(floating).permissions,
                PermissionDisplay::Octal
            );
            c.app.metadata_formats[usize::from(floating)] = None;
        }
        if width == 24 {
            assert!(geometry.metadata_controls.len() > 1);
        }
        c.app.help = true;
        assert!(ui::geometry(&c.app, bounds).metadata_controls.is_empty());
        c.app.help = false;
        c.app.view_menu = Some(0);
        assert!(ui::geometry(&c.app, bounds).metadata_controls.is_empty());
    }
}

fn cached_times(c: &mut Controller) {
    use std::time::{Duration, SystemTime};
    let mut entry = c.app.details[0].entry.clone().unwrap();
    entry.modified = Some(SystemTime::UNIX_EPOCH + Duration::new(10 * 86_400, 123_456_700));
    c.app.now = entry.modified.unwrap() + Duration::from_secs(86_400);
    entry.created = Some(c.app.now - Duration::from_secs(3600));
    entry.accessed = None;
    c.app.complete_listing(
        0,
        1,
        Ok(fs::Snapshot {
            directory: Some(Box::new(fs::inspect(&c.app.panes[0].path))),
            entry_identity: None,
            path: c.app.panes[0].path.clone(),
            entries: vec![entry.clone()],
            errors: vec![],
        }),
    );
    c.app.panes[0].columns.modified = false;
    let request = c.app.request_metadata(0).unwrap();
    assert!(c.app.complete_metadata(&request, entry.clone(), None));
    std::fs::remove_file(entry.path).unwrap();
    assert!(c.app.request_metadata(0).is_none());
}

#[test]
fn metadata_times_follow_pane_mode_and_advance_from_cached_timestamps() {
    for floating in [false, true] {
        let (_root, mut c) = loaded();
        cached_times(&mut c);
        c.app.popup = floating;
        let detail = c.app.details[0].clone();
        c.app.panes[0].fuzzy_timestamps = true;
        let buffer = draw(&c.app, 180, 40, ColorLevel::NoColor);
        find(&buffer, "Modified:");
        find(&buffer, "yesterday");
        find(&buffer, "1 hour ago");
        let (_, accessed) = find(&buffer, "Accessed:");
        assert!(
            (0..180)
                .map(|x| buffer[(x, accessed)].symbol())
                .collect::<String>()
                .contains("unavailable")
        );
        c.app.now += std::time::Duration::from_secs(86_400);
        find(&draw(&c.app, 180, 40, ColorLevel::NoColor), "2 days ago");
        c.app.panes[0].fuzzy_timestamps = false;
        let buffer = draw(&c.app, 180, 40, ColorLevel::NoColor);
        find(&buffer, "Modified:");
        let geometry = ui::geometry(&c.app, Rect::new(0, 0, 180, 40));
        visible_value(&buffer, &geometry, "1970-01-11 00:00:00.123456700 (UTC)");
        find(&buffer, "Created:");
        find(&buffer, "Accessed:");
        assert_eq!(c.app.details[0].path, detail.path);
        assert_eq!(c.app.details[0].generation, detail.generation);
        assert_eq!(
            c.app.details[0].entry.as_ref().unwrap().modified,
            detail.entry.unwrap().modified
        );
        assert!(c.app.request_metadata(0).is_none());
    }
}

#[test]
fn each_metadata_time_value_toggles_only_the_open_metadata_view() {
    for floating in [false, true] {
        for (label, text) in [
            ("Modified:", "1970-01-11 00:00:00.123456700 (UTC)"),
            ("Created:", "1970-01-11 23:00:00.123456700 (UTC)"),
            ("Accessed:", "unavailable"),
        ] {
            let (_root, mut c) = loaded();
            cached_times(&mut c);
            c.app.popup = floating;
            let buffer = draw(&c.app, 180, 40, ColorLevel::NoColor);
            let geometry = ui::geometry(&c.app, Rect::new(0, 0, 180, 40));
            let (x, y) = visible_value(&buffer, &geometry, text);
            click(&mut c, x, y, &geometry);
            assert!(
                !c.app.panes[0].fuzzy_timestamps && !c.preferences.fuzzy_timestamps,
                "{floating} {label} {x},{y} controls={:?}",
                geometry.metadata_controls
            );
            assert!(!c.app.panes[1].fuzzy_timestamps);
            assert!(!c.preferences_changed && !c.app.panes[0].profile_modified);
            let settings = terminal_file_manager::settings::Settings::parse(
                &toml::to_string(&c.preferences).unwrap(),
            )
            .unwrap();
            assert!(!settings.fuzzy_timestamps);
            assert!(c.app.metadata_format(floating).fuzzy_timestamps);
            assert!(c.app.request_metadata(0).is_none());
            let buffer = draw(&c.app, 180, 40, ColorLevel::NoColor);
            let geometry = ui::geometry(&c.app, Rect::new(0, 0, 180, 40));
            let (x, y) = visible_value(&buffer, &geometry, "yesterday");
            click(&mut c, x, y, &geometry);
            assert!(!c.app.panes[0].fuzzy_timestamps && !c.preferences.fuzzy_timestamps);
        }
    }
}

fn show_time_tail(c: &mut Controller, bounds: Rect, floating: bool, tail: &str) -> Rect {
    use terminal_file_manager::input::Action;
    let geometry = ui::geometry(&c.app, bounds);
    let limit = if floating {
        geometry.overlay_scroll_limit
    } else {
        geometry.metadata_scroll_limit
    };
    (0..=limit)
        .find_map(|scroll| {
            if floating {
                c.app.overlay_scroll = scroll;
            } else {
                c.app.metadata_scroll = scroll;
            }
            let buffer = draw(&c.app, bounds.width, bounds.height, ColorLevel::NoColor);
            ui::geometry(&c.app, bounds)
                .metadata_controls
                .into_iter()
                .find_map(|control| {
                    let value: String = (control.area.x..control.area.right())
                        .map(|x| buffer[(x, control.area.y)].symbol())
                        .collect();
                    (control.action == Action::FuzzyTimestamps && value == tail)
                        .then_some(control.area)
                })
        })
        .expect("visible wrapped timestamp continuation")
}

#[test]
fn timestamp_continuations_remain_clickable_after_narrow_overlay_and_sidebar_scroll() {
    for (floating, width, tail) in [
        (true, 24, "00 (UTC)"),
        (true, 40, "23456700 (UTC)"),
        (false, 120, "6700 (UTC)"),
    ] {
        let (_root, mut c) = loaded();
        cached_times(&mut c);
        c.app.popup = floating;
        let bounds = Rect::new(0, 0, width, 12);
        let position = show_time_tail(&mut c, bounds, floating, tail);
        let geometry = ui::geometry(&c.app, bounds);
        click(&mut c, position.x - 1, position.y, &geometry);
        click(&mut c, position.right(), position.y, &geometry);
        assert!(!c.app.panes[0].fuzzy_timestamps);
        c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Right),
                column: position.x,
                row: position.y,
                modifiers: KeyModifiers::NONE,
            }),
            &geometry,
            std::time::Duration::ZERO,
        );
        assert!(!c.app.panes[0].fuzzy_timestamps);
        click(&mut c, position.x, position.y, &geometry);
        assert!(!c.app.panes[0].fuzzy_timestamps && !c.preferences.fuzzy_timestamps);
        assert!(c.app.metadata_format(floating).fuzzy_timestamps);
        assert!(c.app.request_metadata(0).is_none());
    }
}

#[test]
fn native_attribute_text_survives_both_formats_and_excess_scroll_resize() {
    let (_root, mut c) = loaded();
    let entry = c.app.details[0].entry.as_mut().unwrap();
    entry.mode = None;
    entry.attributes = "readonly, hidden".into();
    for format in [PermissionDisplay::Symbolic, PermissionDisplay::Octal] {
        c.app.panes[0].permissions = format;
        c.app.popup = true;
        c.app.overlay_scroll = u16::MAX;
        for (width, height) in [(24, 10), (120, 40)] {
            let buffer = draw(&c.app, width, height, ColorLevel::NoColor);
            let geometry = ui::geometry(&c.app, Rect::new(0, 0, width, height));
            for control in &geometry.metadata_controls {
                assert!(buffer.area.contains(control.area.as_position()));
            }
            if width == 120 {
                find(&buffer, "readonly, hidden");
            }
        }
    }
}

#[test]
fn ordinary_entries_omit_link_target_and_link_objects_retain_unavailable_detail() {
    for floating in [false, true] {
        let (_root, mut c) = loaded();
        c.app.popup = floating;
        for kind in [
            fs::EntryKind::File,
            fs::EntryKind::Directory,
            fs::EntryKind::Link,
            fs::EntryKind::Junction,
            fs::EntryKind::ReparsePoint,
        ] {
            c.app.details[0].entry.as_mut().unwrap().kind = kind;
            let buffer = draw(&c.app, 180, 40, ColorLevel::NoColor);
            let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
            assert_eq!(
                text.contains("Link target:"),
                !matches!(kind, fs::EntryKind::File | fs::EntryKind::Directory)
            );
        }
    }
}

fn owner_continuation(c: &mut Controller, bounds: Rect, floating: bool) -> Rect {
    use terminal_file_manager::input::Action;
    let g = ui::geometry(&c.app, bounds);
    let limit = if floating {
        g.overlay_scroll_limit
    } else {
        g.metadata_scroll_limit
    };
    for scroll in 0..=limit {
        if floating {
            c.app.overlay_scroll = scroll;
        } else {
            c.app.metadata_scroll = scroll;
        }
        let controls: Vec<_> = ui::geometry(&c.app, bounds)
            .metadata_controls
            .into_iter()
            .filter(|control| control.action == Action::OwnerDisplay)
            .collect();
        if controls.len() > 1 {
            return controls[1].area;
        }
    }
    panic!("wrapped owner continuation");
}
#[test]
fn owner_values_and_continuations_exclude_labels_padding_and_borders() {
    use terminal_file_manager::{
        input::Action,
        owner::{OwnerDisplay, Ownership},
    };
    for (floating, width) in [(true, 24), (true, 40), (false, 120)] {
        let (_root, mut c) = loaded();
        c.app.popup = floating;
        c.app.details[0].entry.as_mut().unwrap().owner = Some(Ownership {
            uid: 12,
            gid: 34,
            user: "cached-user-".repeat(6),
            group: "cached-group".into(),
        });
        let bounds = Rect::new(0, 0, width, 12);
        let value = owner_continuation(&mut c, bounds, floating);
        let g = ui::geometry(&c.app, bounds);
        click(&mut c, value.x - 1, value.y, &g);
        click(&mut c, value.right(), value.y, &g);
        assert_eq!(
            c.app.metadata_format(floating).owner_display,
            OwnerDisplay::Names
        );
        click(&mut c, value.x, value.y, &g);
        assert_eq!(
            c.app.metadata_format(floating).owner_display,
            OwnerDisplay::Numeric
        );
        assert_eq!(c.app.panes[0].owner_display, OwnerDisplay::Names);
        assert_eq!(c.preferences.owner_display, OwnerDisplay::Names);
        assert!(!c.preferences_changed);
        assert!(
            g.metadata_controls
                .iter()
                .any(|control| control.action == Action::OwnerDisplay)
        );
    }
}
