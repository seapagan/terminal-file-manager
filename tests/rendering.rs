//! Ratatui buffers prove adaptive layout and non-colour state indicators.
use colored_text::ColorLevel;
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};
use std::{fs, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    fs::{IgnoreContext, list},
    ui,
};

#[test]
fn named_palette_maps_normal_and_bright_cells_without_ansi_strings() {
    use colored_text::{AnsiColor::*, ResolvedColor};
    for (named, expected) in [
        (Black, Color::Black),
        (Red, Color::Red),
        (Green, Color::Green),
        (Yellow, Color::Yellow),
        (Blue, Color::Blue),
        (Magenta, Color::Magenta),
        (Cyan, Color::Cyan),
        (White, Color::Gray),
        (BrightBlack, Color::DarkGray),
        (BrightRed, Color::LightRed),
        (BrightGreen, Color::LightGreen),
        (BrightYellow, Color::LightYellow),
        (BrightBlue, Color::LightBlue),
        (BrightMagenta, Color::LightMagenta),
        (BrightCyan, Color::LightCyan),
        (BrightWhite, Color::White),
    ] {
        assert_eq!(
            terminal_file_manager::color::map_color(ResolvedColor::Named(named)),
            expected
        );
    }
}

#[test]
fn theme_resolves_all_capability_levels_and_keeps_noncolour_cursor() {
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let theme = Theme::new(level);
        let style = theme.style(Role::Cursor);
        assert!(style.add_modifier.contains(Modifier::REVERSED));
        let directory = theme.style(Role::Directory);
        assert_eq!(directory.fg.is_none(), level == ColorLevel::NoColor);
        let custom = theme.resolve("#2468ab").unwrap();
        match level {
            ColorLevel::NoColor => assert_eq!(custom, None),
            ColorLevel::Ansi16 => {
                assert!(!matches!(custom, Some(Color::Indexed(_) | Color::Rgb(..))))
            }
            ColorLevel::Ansi256 => assert!(matches!(custom, Some(Color::Indexed(_)))),
            ColorLevel::TrueColor => assert_eq!(custom, Some(Color::Rgb(0x24, 0x68, 0xab))),
        }
    }
}

#[test]
fn dual_pane_focus_distinguishes_retained_cursor_and_background() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("selected"), "one").unwrap();
    let mut app = App::new(root.path().to_path_buf());
    for id in 0..2 {
        let snapshot = list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap();
        app.complete_listing(id, 1, Ok(snapshot));
    }
    app.dual = true;
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        for active in 0..2 {
            app.active = active;
            let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();
            terminal
                .draw(|frame| {
                    ui::render(frame, &app, &Theme::new(level));
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            let focused = &buffer[(1 + active as u16 * 60, 2)];
            let retained = &buffer[(1 + (1 - active) as u16 * 60, 2)];
            assert!(
                focused
                    .modifier
                    .contains(Modifier::REVERSED | Modifier::BOLD)
            );
            assert!(!focused.modifier.contains(Modifier::DIM));
            assert!(retained.modifier.contains(Modifier::DIM));
            assert!(!retained.modifier.contains(Modifier::REVERSED));
            let active_space = &buffer[(58 + active as u16 * 60, 10)];
            let inactive_space = &buffer[(58 + (1 - active) as u16 * 60, 10)];
            assert_eq!(
                active_space.bg == inactive_space.bg,
                level == ColorLevel::NoColor
            );
        }
    }
}

#[test]
fn tiny_unicode_and_control_names_render_without_escapes_or_losing_focus() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("界e\u{301} name"), "one").unwrap();
    #[cfg(unix)]
    fs::write(root.path().join("bad\u{1b}[31m\nname"), "two").unwrap();
    let mut app = App::new(root.path().to_path_buf());
    let snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    app.complete_listing(0, 1, Ok(snapshot));
    app.panes[0].select_all();
    app.dual = true;
    app.sidebar = true;
    for (width, height) in [(1, 1), (20, 5), (79, 12), (120, 24), (200, 40)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                ui::render(frame, &app, &Theme::new(ColorLevel::NoColor));
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(
            buffer
                .content()
                .iter()
                .all(|c| !c.symbol().contains('\u{1b}'))
        );
        assert!(
            buffer
                .content()
                .iter()
                .all(|c| c.fg == Color::Reset && c.bg == Color::Reset)
        );
        if width >= 79 {
            assert!(
                buffer
                    .content()
                    .iter()
                    .any(|c| c.modifier.contains(Modifier::REVERSED))
            );
        }
    }
}

#[test]
fn layout_restores_sidebar_preference_and_dual_view_after_resize() {
    let mut app = App::new("/example".into());
    app.sidebar = true;
    app.sidebar_left = true;
    app.active = 1;
    let wide = ui::geometry(&app, ratatui::layout::Rect::new(0, 0, 120, 24));
    assert!(wide.sidebar.is_some());
    assert!(wide.panes[1].is_some());
    assert!(
        ui::geometry(&app, ratatui::layout::Rect::new(0, 0, 30, 8))
            .sidebar
            .is_none()
    );
    app.dual = true;
    let dual = ui::geometry(&app, ratatui::layout::Rect::new(0, 0, 120, 24));
    assert!(dual.sidebar.is_none());
    assert!(dual.panes.iter().all(Option::is_some));
    app.dual = false;
    assert!(
        ui::geometry(&app, ratatui::layout::Rect::new(0, 0, 120, 24))
            .sidebar
            .is_some()
    );
}

#[test]
fn metadata_fields_have_separate_rows_and_share_the_cached_entry() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("selected"), "three").unwrap();
    let mut app = App::new(root.path().to_path_buf());
    let snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    app.complete_listing(0, 1, Ok(snapshot));
    let request = app.request_metadata(0).unwrap();
    app.complete_metadata(
        &request,
        terminal_file_manager::fs::inspect(&request.path),
        None,
    );
    app.popup = true;
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, &app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rows: Vec<String> = buffer
        .content()
        .chunks(120)
        .map(|row| row.iter().map(|c| c.symbol()).collect())
        .collect();
    let name = rows
        .iter()
        .position(|row| row.contains("Name: selected"))
        .unwrap();
    let path = rows.iter().position(|row| row.contains("Path:")).unwrap();
    assert!(path > name);
    assert!(!rows.iter().any(|row| row.contains("\\n")));
}

#[test]
fn compact_size_columns_align_values_and_units_without_a_fill_gap() {
    let root = tempfile::tempdir().unwrap();
    for (name, size) in [("one", 1), ("partial", 1536), ("promoted", 1_048_525)] {
        let file = fs::File::create(root.path().join(name)).unwrap();
        file.set_len(size).unwrap();
    }
    let mut app = App::new(root.path().to_path_buf());
    app.complete_listing(
        0,
        1,
        Ok(list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    let mut terminal = Terminal::new(TestBackend::new(180, 24)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, &app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let row = |y: u16| {
        (0..180)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    };
    let lines = [row(2), row(3), row(4)];
    let first_unit = lines[0].find(" B").unwrap();
    assert_eq!(lines[1].find(" K"), Some(first_unit));
    assert_eq!(lines[2].find(" M"), Some(first_unit));
    assert!(
        first_unit < 35,
        "size should follow the compact filename column"
    );
    assert!(lines[1][..first_unit].ends_with("1.5"));
    assert!(lines[2][..first_unit].ends_with("  1"));
}

#[test]
fn exact_sizes_remeasure_on_settings_and_filter_changes_with_unicode_names() {
    let root = tempfile::tempdir().unwrap();
    for name in ["界e\u{301}", "a much longer filename"] {
        let file = fs::File::create(root.path().join(name)).unwrap();
        file.set_len(1_048_525).unwrap();
    }
    let mut app = App::new(root.path().to_path_buf());
    app.complete_listing(
        0,
        1,
        Ok(list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    assert_eq!(app.panes[0].measurements.number, 4);
    let settings = terminal_file_manager::settings::Settings::parse("human_sizes = false").unwrap();
    settings.apply(&mut app);
    assert_eq!(app.panes[0].measurements.number, 7);
    app.panes[0].set_filter("界".into());
    assert_eq!(app.panes[0].measurements.name, 4);
    for width in [1, 7, 8, 25, 45, 47, 70, 90, 125, 180] {
        let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
        terminal
            .draw(|frame| {
                ui::render(frame, &app, &Theme::new(ColorLevel::NoColor));
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(
            buffer
                .content()
                .iter()
                .all(|cell| !cell.symbol().contains('\u{1b}'))
        );
        if width >= 47 {
            let row = (0..width)
                .map(|x| buffer[(x, 2)].symbol())
                .collect::<String>();
            assert!(row.contains("1048525"));
            assert!(!row.contains(" KiB"));
        }
    }
}

#[test]
fn fuzzy_timestamp_setting_uses_one_fixed_frame_clock_and_preserves_exact_utc() {
    use std::time::{Duration, UNIX_EPOCH};
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("entry"), "one").unwrap();
    let mut app = App::new(root.path().to_path_buf());
    app.complete_listing(
        0,
        1,
        Ok(list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    app.now = UNIX_EPOCH + Duration::from_secs(1_000_000);
    app.panes[0].entries[0].modified = app.now.checked_sub(Duration::from_secs(86_400));
    for (config, expected) in [
        ("fuzzy_timestamps = true", "yesterday"),
        ("fuzzy_timestamps = false", "1970-01-11"),
    ] {
        terminal_file_manager::settings::Settings::parse(config)
            .unwrap()
            .apply(&mut app);
        let mut terminal = Terminal::new(TestBackend::new(100, 12)).unwrap();
        terminal
            .draw(|frame| {
                ui::render(frame, &app, &Theme::new(ColorLevel::NoColor));
            })
            .unwrap();
        let row = (0..100)
            .map(|x| terminal.backend().buffer()[(x, 2)].symbol())
            .collect::<String>();
        assert!(row.contains(expected), "{row}");
    }
}
