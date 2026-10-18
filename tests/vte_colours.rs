//! VTE compatibility changes only ignored names and visible hidden-marker foregrounds.
use colored_text::ColorLevel;
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend, TestBackend},
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier},
};
use std::{fs, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::{App, IgnoreMode},
    color::Theme,
    fs::{IgnoreContext, list},
    ui,
};

const LEVELS: [ColorLevel; 4] = [
    ColorLevel::NoColor,
    ColorLevel::Ansi16,
    ColorLevel::Ansi256,
    ColorLevel::TrueColor,
];

fn fixture() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join(".git/info")).unwrap();
    fs::write(root.path().join(".gitignore"), "ignored-*\n").unwrap();
    for name in ["ignored-dir", "ordinary-dir"] {
        fs::create_dir(root.path().join(name)).unwrap();
    }
    for name in ["ignored-file", "ordinary-file", "selected-file"] {
        fs::write(root.path().join(name), "data").unwrap();
    }
    let mut app = App::new(root.path().into());
    app.dual = true;
    for id in 0..2 {
        app.complete_listing(
            id,
            1,
            Ok(list(
                root.path(),
                &IgnoreContext::isolated(),
                &AtomicBool::new(false),
            )
            .unwrap()),
        );
        app.panes[id].show_hidden = true;
        for entry in &mut app.panes[id].entries {
            if matches!(
                entry.name.to_str(),
                Some("ignored-file" | "ordinary-dir" | "selected-file")
            ) {
                entry.hidden = true;
            }
        }
        app.panes[id].refilter();
        app.panes[id].cursor = Some(root.path().join("selected-file"));
        app.panes[id].select_all();
    }
    (root, app)
}

fn draw(terminal: &mut Terminal<TestBackend>, app: &App, theme: &Theme, width: u16) -> Buffer {
    terminal.backend_mut().resize(width, 18);
    terminal.resize(Rect::new(0, 0, width, 18)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, theme);
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

#[test]
fn only_vte_truecolor_names_and_hidden_markers_change_after_focus_selection_and_resize() {
    let (root, mut app) = fixture();
    let mut terminal = Terminal::new(TestBackend::new(240, 18)).unwrap();
    for level in LEVELS {
        for active in 0..2 {
            app.active = active;
            for cursor in ["selected-file", "ignored-file", "ignored-dir"] {
                for pane in &mut app.panes {
                    pane.cursor = Some(root.path().join(cursor));
                }
                for width in [240, 80, 40, 7, 180] {
                    let baseline = draw(&mut terminal, &app, &Theme::new(level), width);
                    for version in [
                        None,
                        Some("8400"),
                        Some(""),
                        Some("bad"),
                        Some("0"),
                        Some("-8400"),
                        Some("+8400"),
                        Some(" 8400"),
                        Some("4294967296"),
                    ] {
                        let actual = draw(
                            &mut terminal,
                            &app,
                            &Theme::with_vte_version(level, version),
                            width,
                        );
                        let mut expected = baseline.clone();
                        if version == Some("8400") && level == ColorLevel::TrueColor {
                            palette_names(&mut expected, &app);
                            palette_hidden_markers(&mut expected, &app);
                        }
                        assert_eq!(
                            actual, expected,
                            "{level:?} {version:?} active={active} cursor={cursor} width={width}"
                        );
                    }
                }
            }
        }
    }
}

fn palette_names(buffer: &mut Buffer, app: &App) {
    let geometry = ui::geometry(app, buffer.area);
    for (id, area) in geometry.panes.iter().enumerate() {
        let Some(area) = area else { continue };
        let start = area.x + if area.width < 10 { 1 } else { 8 };
        for (row, index) in app.panes[id].visible.iter().enumerate() {
            let entry = &app.panes[id].entries[*index];
            if entry.ignored != Some(true) || app.panes[id].cursor.as_ref() == Some(&entry.path) {
                continue;
            }
            let y = area.y + 2 + row as u16;
            for x in start..area.right() - 1 {
                let cell = &mut buffer[(x, y)];
                if cell.symbol() == " " {
                    break;
                }
                cell.fg = match cell.fg {
                    Color::Rgb(70, 120, 185) => Color::Indexed(67),
                    Color::Rgb(220, 225, 230) => Color::Indexed(254),
                    other => other,
                };
            }
        }
    }
}

#[test]
fn vte_off_and_hide_modes_change_only_visible_unfocused_hidden_markers() {
    let (_root, mut app) = fixture();
    let mut terminal = Terminal::new(TestBackend::new(180, 18)).unwrap();
    for mode in [IgnoreMode::Off, IgnoreMode::Hide] {
        for pane in &mut app.panes {
            pane.ignore = mode;
            pane.refilter();
        }
        for level in LEVELS {
            let baseline = draw(&mut terminal, &app, &Theme::new(level), 180);
            let vte = draw(
                &mut terminal,
                &app,
                &Theme::with_vte_version(level, Some("8400")),
                180,
            );
            let mut expected = baseline;
            if level == ColorLevel::TrueColor {
                palette_hidden_markers(&mut expected, &app);
            }
            assert_eq!(vte, expected);
        }
    }
}

#[test]
fn ignored_names_and_hidden_markers_emit_palette_or_rgb_faint_and_reset_before_padding() {
    crossterm::style::force_color_output(true);
    let (_root, mut app) = fixture();
    let mut terminal = Terminal::new(TestBackend::new(180, 18)).unwrap();
    for level in LEVELS {
        for active in 0..2 {
            app.active = active;
            for version in [None, Some("8400")] {
                let buffer = draw(
                    &mut terminal,
                    &app,
                    &Theme::with_vte_version(level, version),
                    180,
                );
                for id in 0..2 {
                    assert_names_and_output(&buffer, &app, id, level, version.is_some());
                    assert_hidden_output(&buffer, &app, id, level, version.is_some());
                }
            }
        }
    }
}

fn assert_names_and_output(buffer: &Buffer, app: &App, id: usize, level: ColorLevel, vte: bool) {
    let area = ui::geometry(app, buffer.area).panes[id].unwrap();
    for (row, index) in app.panes[id].visible.iter().enumerate() {
        let entry = &app.panes[id].entries[*index];
        if entry.ignored != Some(true) {
            continue;
        }
        let x = area.x + 8;
        let y = area.y + 2 + row as u16;
        let cell = &buffer[(x, y)];
        let directory = entry.name == "ignored-dir";
        let fg = expected_foreground(level, vte, id == app.active, directory);
        assert_eq!(cell.fg, fg);
        assert_eq!(cell.modifier, Modifier::DIM);
        let length = entry.name.to_str().unwrap().len() as u16 + u16::from(directory);
        let mut bytes = Vec::new();
        CrosstermBackend::new(&mut bytes)
            .draw((x..x + length + 1).map(|x| (x, y, &buffer[(x, y)])))
            .unwrap();
        let output = String::from_utf8(bytes).unwrap();
        assert!(
            output.starts_with(&format!("\x1b[{};{}H\x1b[2m", y + 1, x + 1)),
            "{output:?}"
        );
        let name_end = output.find(entry.name.to_str().unwrap()).unwrap() + length as usize;
        assert!(output[name_end..].starts_with("\x1b[22m"), "{output:?}");
        assert!(output.ends_with("\x1b[0m"), "{output:?}");
        assert_foreground_sequence(&output[..name_end], fg);
        assert!(!buffer[(x + length, y)].modifier.contains(Modifier::DIM));
    }
}

fn expected_foreground(level: ColorLevel, vte: bool, active: bool, directory: bool) -> Color {
    match (level, vte, active, directory) {
        (ColorLevel::NoColor, _, _, _) => Color::Reset,
        (ColorLevel::Ansi16, _, _, true) => Color::Blue,
        (ColorLevel::Ansi256, _, _, true) | (ColorLevel::TrueColor, true, _, true) => {
            Color::Indexed(67)
        }
        (ColorLevel::Ansi256, _, true, false) | (ColorLevel::TrueColor, true, true, false) => {
            Color::Indexed(254)
        }
        (ColorLevel::TrueColor, false, _, true) => Color::Rgb(70, 120, 185),
        (ColorLevel::TrueColor, false, true, false) => Color::Rgb(220, 225, 230),
        _ => Color::Reset,
    }
}

fn assert_foreground_sequence(output: &str, foreground: Color) {
    let sequence = match foreground {
        Color::Rgb(r, g, b) => format!("38;2;{r};{g};{b}"),
        Color::Indexed(n) => format!("38;5;{n}"),
        Color::Blue => "38;5;4".into(),
        Color::Cyan => "38;5;6".into(),
        Color::Reset => return,
        other => panic!("unexpected foreground {other:?}"),
    };
    assert!(output.contains(&sequence), "{output:?}");
}

fn palette_hidden_markers(buffer: &mut Buffer, app: &App) {
    let geometry = ui::geometry(app, buffer.area);
    for (id, area) in geometry.panes.iter().enumerate() {
        let Some(area) = area.filter(|area| area.width >= 10) else {
            continue;
        };
        for (row, index) in app.panes[id].visible.iter().enumerate() {
            let entry = &app.panes[id].entries[*index];
            if entry.hidden && app.panes[id].cursor.as_ref() != Some(&entry.path) {
                let cell = &mut buffer[(area.x + 6, area.y + 2 + row as u16)];
                assert_eq!(cell.symbol(), "h");
                assert_eq!(cell.fg, Color::Rgb(35, 140, 150));
                assert_eq!(cell.modifier, Modifier::DIM);
                cell.fg = Color::Indexed(30);
            }
        }
    }
}

fn assert_hidden_output(buffer: &Buffer, app: &App, id: usize, level: ColorLevel, vte: bool) {
    let area = ui::geometry(app, buffer.area).panes[id].unwrap();
    for (row, index) in app.panes[id].visible.iter().enumerate() {
        let entry = &app.panes[id].entries[*index];
        if !entry.hidden || app.panes[id].cursor.as_ref() == Some(&entry.path) {
            continue;
        }
        let (x, y) = (area.x + 6, area.y + 2 + row as u16);
        let cell = &buffer[(x, y)];
        let fg = match (level, vte) {
            (ColorLevel::NoColor, _) => Color::Reset,
            (ColorLevel::Ansi16, _) => Color::Cyan,
            (ColorLevel::Ansi256, _) | (ColorLevel::TrueColor, true) => Color::Indexed(30),
            _ => Color::Rgb(35, 140, 150),
        };
        assert_eq!(cell.symbol(), "h");
        assert_eq!(cell.fg, fg);
        assert_eq!(cell.modifier, Modifier::DIM);
        let mut bytes = Vec::new();
        CrosstermBackend::new(&mut bytes)
            .draw((x..x + 2).map(|x| (x, y, &buffer[(x, y)])))
            .unwrap();
        let output = String::from_utf8(bytes).unwrap();
        assert!(
            output.starts_with(&format!("\x1b[{};{}H\x1b[2m", y + 1, x + 1)),
            "{output:?}"
        );
        assert_foreground_sequence(&output, fg);
        assert!(output.contains("h\x1b[22m"), "{output:?}");
        assert!(!buffer[(x + 1, y)].modifier.contains(Modifier::DIM));
        assert!(output.ends_with("\x1b[0m"));
    }
}
