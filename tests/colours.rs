//! Ratatui adaptations of lsplus name, size, age and permission colour cases.
use colored_text::ColorLevel;
use ratatui::style::{Color, Modifier};
use ratatui::{Terminal, backend::TestBackend};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, UNIX_EPOCH};
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs::{self, EntryKind},
    ui,
};

#[test]
fn lsplus_type_accents_preserve_semantics_without_ansi_strings() {
    let root = tempfile::tempdir().unwrap();
    let mut entry = fs::inspect(&root.path().join("fixture"));
    entry.error = None;
    let theme = Theme::new(ColorLevel::Ansi16);
    for (kind, colour, bold) in [
        (EntryKind::Directory, Some(Color::Blue), false),
        (EntryKind::Link, Some(Color::Cyan), false),
        (EntryKind::Junction, Some(Color::Magenta), false),
        (EntryKind::Socket, Some(Color::Magenta), true),
        (EntryKind::Fifo, Some(Color::Yellow), false),
        (EntryKind::Device, Some(Color::Yellow), true),
        (EntryKind::File, None, false),
    ] {
        entry.kind = kind;
        let style = theme.entry_style(&entry);
        assert_eq!(style.fg, colour);
        assert_eq!(style.add_modifier.contains(Modifier::BOLD), bold);
    }
    entry.kind = EntryKind::File;
    entry.executable = true;
    assert_eq!(theme.entry_style(&entry).fg, Some(Color::Green));
    assert!(
        theme
            .entry_style(&entry)
            .add_modifier
            .contains(Modifier::BOLD)
    );
}

#[test]
fn size_and_age_boundaries_follow_lsplus_and_downgrade_without_colours() {
    let now = UNIX_EPOCH + Duration::from_secs(100_000_000);
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let theme = Theme::new(level);
        assert!(theme.size_style(Some(1_048_575)).fg.is_none());
        for bytes in [1_048_576, 1_073_741_823, 1_073_741_824] {
            assert_eq!(
                theme.size_style(Some(bytes)).fg.is_none(),
                level == ColorLevel::NoColor
            );
        }
        let future = theme.time_style(Some(now + Duration::from_secs(1)), now);
        assert_eq!(future.fg.is_none(), level == ColorLevel::NoColor);
        if level == ColorLevel::Ansi16 {
            assert_eq!(future.fg, Some(Color::Red));
            assert!(future.add_modifier.contains(Modifier::BOLD));
        }
        for age in [0, 86_399, 86_400, 604_800, 2_592_000, 31_536_000] {
            let style = theme.time_style(Some(now - Duration::from_secs(age)), now);
            assert_eq!(style.fg.is_none(), level == ColorLevel::NoColor);
        }
    }
}

#[test]
fn ignored_hidden_executable_and_mark_cues_compose_beneath_active_cursor() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(".run"), "data").unwrap();
    let mut app = App::new(root.path().to_path_buf());
    app.complete_listing(
        0,
        1,
        Ok(fs::list(
            root.path(),
            &fs::IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    let pane = &mut app.panes[0];
    pane.entries[0].hidden = true;
    pane.entries[0].ignored = Some(true);
    pane.entries[0].executable = true;
    pane.show_hidden = true;
    pane.refilter();
    pane.select_all();
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        for cursor in [false, true] {
            app.panes[0].cursor = cursor.then(|| root.path().join(".run"));
            let mut terminal = Terminal::new(TestBackend::new(100, 12)).unwrap();
            terminal
                .draw(|frame| {
                    ui::render(frame, &app, &Theme::new(level));
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            let name = &buffer[(11, 2)];
            assert_eq!(name.symbol(), ".");
            assert_eq!(name.modifier.contains(Modifier::DIM), !cursor);
            assert_eq!(name.modifier.contains(Modifier::REVERSED), cursor);
            assert!(name.modifier.contains(Modifier::BOLD));
            if level != ColorLevel::NoColor && !cursor {
                let expected = Theme::new(level).entry_style(&app.panes[0].entries[0]).fg;
                assert_eq!(Some(name.fg), expected);
            }
            assert_eq!(buffer[(2, 2)].symbol(), "*");
            assert_eq!(buffer[(7, 2)].symbol(), "[");
        }
    }
}

#[test]
fn permission_spans_and_age_palette_match_original_lsplus_cases() {
    let theme = Theme::new(ColorLevel::Ansi16);
    for (value, colour, bold, dim) in [
        ('r', Some(Color::Green), false, false),
        ('w', Some(Color::Yellow), false, false),
        ('x', Some(Color::Red), true, false),
        ('s', Some(Color::Red), true, false),
        ('t', Some(Color::Red), true, false),
        ('S', None, false, true),
        ('T', None, false, true),
        ('-', None, false, true),
    ] {
        let style = theme.permission_style(value);
        assert_eq!(style.fg, colour);
        assert_eq!(style.add_modifier.contains(Modifier::BOLD), bold);
        assert_eq!(style.add_modifier.contains(Modifier::DIM), dim);
    }
    let theme = Theme::new(ColorLevel::Ansi256);
    let now = UNIX_EPOCH + Duration::from_secs(100_000_000);
    for (age, index) in [
        (0, 222),
        (86_399, 222),
        (86_400, 221),
        (604_800, 178),
        (2_592_000, 136),
        (31_536_000, 130),
    ] {
        assert_eq!(
            theme
                .time_style(Some(now - Duration::from_secs(age)), now)
                .fg,
            Some(Color::Indexed(index))
        );
    }
}
