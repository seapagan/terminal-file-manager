//! Ratatui adaptations of lsplus name, size, age and permission colour cases.
use colored_text::ColorLevel;
use ratatui::style::{Color, Modifier, Style};
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
    std::fs::write(root.path().join("fixture"), "data").unwrap();
    let mut app = ignored_executable_app(root.path());
    app.dual = true;
    app.active = 1;
    app.panes[0].cursor = None;
    app.panes[0].marks.clear();
    for level in LEVELS {
        for (kind, colour, bold) in [
            (EntryKind::Directory, Some(Color::Blue), false),
            (EntryKind::Link, Some(Color::Cyan), false),
            (EntryKind::Junction, Some(Color::Magenta), false),
            (EntryKind::Socket, Some(Color::Magenta), true),
            (EntryKind::Fifo, Some(Color::Yellow), false),
            (EntryKind::Device, Some(Color::Yellow), true),
            (EntryKind::File, None, false),
            (EntryKind::File, Some(Color::Green), true),
            (EntryKind::ReparsePoint, None, false),
            (EntryKind::Unknown, None, false),
        ] {
            let entry = &mut app.panes[0].entries[0];
            entry.kind = kind;
            entry.executable = colour == Some(Color::Green);
            entry.ignored = Some(false);
            entry.hidden = false;
            entry.error = None;
            assert_entry_cells(&app, level, expected_style(level, colour, bold, false));
            app.panes[0].entries[0].error = Some("unavailable".into());
            assert_entry_cells(
                &app,
                level,
                expected_style(level, Some(Color::Yellow), false, false),
            );
        }
    }
}

fn assert_entry_cells(app: &App, level: ColorLevel, expected: Style) {
    let theme = Theme::new(level);
    assert_eq!(theme.entry_style(&app.panes[0].entries[0]), expected);
    let mut terminal = Terminal::new(TestBackend::new(240, 12)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &theme);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(8, 2)].symbol(), "f");
    for x in [5, 8] {
        let cell = &buffer[(x, 2)];
        assert_eq!(cell.fg, expected.fg.unwrap_or(Color::Reset));
        assert_eq!(cell.bg, Color::Reset);
        assert_eq!(cell.modifier, expected.add_modifier);
    }
}

const LEVELS: [ColorLevel; 4] = [
    ColorLevel::NoColor,
    ColorLevel::Ansi16,
    ColorLevel::Ansi256,
    ColorLevel::TrueColor,
];

fn expected_style(level: ColorLevel, colour: Option<Color>, bold: bool, dim: bool) -> Style {
    let fg = colour.and_then(|named| {
        let (rgb, index) = match named {
            Color::Red => (Color::Rgb(190, 65, 70), 131),
            Color::Green => (Color::Rgb(50, 145, 90), 65),
            Color::Yellow => (Color::Rgb(165, 125, 35), 136),
            Color::Blue => (Color::Rgb(70, 120, 185), 67),
            Color::Magenta => (Color::Rgb(155, 90, 160), 97),
            Color::Cyan => (Color::Rgb(35, 140, 150), 30),
            _ => panic!("unexpected oracle colour"),
        };
        match level {
            ColorLevel::NoColor => None,
            ColorLevel::Ansi16 => Some(named),
            ColorLevel::Ansi256 => Some(Color::Indexed(index)),
            ColorLevel::TrueColor => Some(rgb),
        }
    });
    let mut style = Style {
        fg,
        ..Style::default()
    };
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if dim {
        style = style.add_modifier(Modifier::DIM);
    }
    style
}

#[test]
fn size_and_age_boundaries_follow_lsplus_and_downgrade_without_colours() {
    for level in LEVELS {
        let theme = Theme::new(level);
        assert_size_boundaries(&theme);
        assert_age_boundaries(&theme);
    }
}

fn assert_size_boundaries(theme: &Theme) {
    for (bytes, colour, bold) in [
        (None, None, false),
        (Some(0), None, false),
        (Some(1_048_575), None, false),
        (Some(1_048_576), Some(Color::Yellow), false),
        (Some(1_048_577), Some(Color::Yellow), false),
        (Some(1_073_741_823), Some(Color::Yellow), false),
        (Some(1_073_741_824), Some(Color::Red), true),
        (Some(1_073_741_825), Some(Color::Red), true),
    ] {
        let expected = expected_style(
            theme.level,
            colour,
            bold && theme.level != ColorLevel::NoColor,
            false,
        );
        assert_eq!(theme.size_style(bytes), expected, "{bytes:?}");
    }
}

fn assert_age_boundaries(theme: &Theme) {
    let now = UNIX_EPOCH + Duration::from_secs(100_000_000);
    let level = theme.level;
    assert_eq!(
        theme.time_style(None, now),
        Style::default().add_modifier(Modifier::DIM)
    );
    assert_eq!(
        theme.time_style(Some(now + Duration::from_secs(1)), now),
        expected_style(level, Some(Color::Red), level != ColorLevel::NoColor, false)
    );
    for (age, index) in [
        (0, 222),
        (86_399, 222),
        (86_400, 221),
        (86_401, 221),
        (604_799, 221),
        (604_800, 178),
        (604_801, 178),
        (2_591_999, 178),
        (2_592_000, 136),
        (2_592_001, 136),
        (31_535_999, 136),
        (31_536_000, 130),
        (31_536_001, 130),
    ] {
        let mut expected = expected_style(
            level,
            Some(Color::Yellow),
            age < 86_400 && level != ColorLevel::NoColor,
            level == ColorLevel::Ansi16 && age >= 31_536_000,
        );
        if level >= ColorLevel::Ansi256 {
            expected.fg = Some(Color::Indexed(index));
        }
        assert_eq!(
            theme.time_style(Some(now - Duration::from_secs(age)), now),
            expected,
            "{level:?} {age}"
        );
    }
}

#[test]
fn ignored_hidden_executable_and_mark_cues_compose_beneath_active_cursor() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(".run"), "data").unwrap();
    let mut app = ignored_executable_app(root.path());
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
            let name = &buffer[(8, 2)];
            assert_eq!(name.symbol(), ".");
            assert_eq!(name.modifier.contains(Modifier::DIM), !cursor);
            assert_eq!(
                name.modifier.contains(Modifier::REVERSED),
                cursor && level == ColorLevel::NoColor
            );
            assert!(name.modifier.contains(Modifier::BOLD));
            if level != ColorLevel::NoColor && !cursor {
                let expected = Theme::new(level).entry_style(&app.panes[0].entries[0]).fg;
                assert_eq!(Some(name.fg), expected);
            }
            assert_eq!(buffer[(2, 2)].symbol(), "*");
            assert_eq!(buffer[(5, 2)].symbol(), "*");
            assert_eq!(buffer[(6, 2)].symbol(), "h");
        }
    }
}

#[test]
fn permission_spans_preserve_exact_styles_at_each_colour_depth() {
    for level in LEVELS {
        let theme = Theme::new(level);
        for (value, colour, bold, dim) in [
            ('r', Some(Color::Green), false, false),
            ('w', Some(Color::Yellow), false, false),
            ('x', Some(Color::Red), true, false),
            ('s', Some(Color::Red), true, false),
            ('t', Some(Color::Red), true, false),
            ('S', None, false, true),
            ('T', None, false, true),
            ('-', None, false, true),
            ('?', None, false, false),
        ] {
            assert_eq!(
                theme.permission_style(value),
                expected_style(level, colour, bold, dim)
            );
        }
    }
}

fn ignored_executable_app(root: &std::path::Path) -> App {
    let mut app = App::new(root.to_path_buf());
    app.complete_listing(
        0,
        1,
        Ok(fs::list(
            root,
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
    app
}
