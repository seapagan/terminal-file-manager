use super::*;

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
