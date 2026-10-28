use super::*;

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
