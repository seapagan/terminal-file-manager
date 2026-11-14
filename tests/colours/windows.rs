//! System filenames use cached native attributes without changing other row cues.
use super::*;
use ratatui::buffer::Buffer;
use std::{path::Path, process::Command};

fn system_fixture(directory: bool) -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fixture");
    if directory {
        std::fs::create_dir(&path).unwrap();
    } else {
        std::fs::write(&path, "data").unwrap();
    }
    attribute(&path, "+S");
    let snapshot = fs::list(
        root.path(),
        &fs::IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_ne!(snapshot.entries[0].windows_attributes.unwrap() & 4, 0);
    attribute(&path, "-S");
    if directory {
        std::fs::remove_dir(&path).unwrap();
    } else {
        std::fs::remove_file(&path).unwrap();
    }
    let mut app = App::new(root.path().into());
    app.dual = true;
    for id in 0..2 {
        app.complete_listing(id, 1, Ok(snapshot.clone()));
        let pane = &mut app.panes[id];
        pane.entries[0].hidden = true;
        pane.entries[0].executable = !directory;
        pane.show_hidden = true;
        pane.columns.attributes = false;
        pane.refilter();
    }
    (root, app)
}

fn attribute(path: &Path, value: &str) {
    let output = Command::new("attrib")
        .arg(value)
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

fn draw(app: &App, theme: &Theme, width: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, theme);
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn set_state(app: &mut App, state: u8) {
    for pane in &mut app.panes {
        pane.entries[0].ignored = Some(state & 1 != 0);
        pane.cursor = (state & 2 != 0).then(|| pane.entries[0].path.clone());
        pane.marks.clear();
        if state & 4 != 0 {
            pane.select_all();
        }
    }
}

fn compare_system_names(app: &mut App, theme: &Theme, width: u16, red: Option<Color>) {
    for pane in &mut app.panes {
        pane.entries[0].windows_attributes = Some(0);
    }
    let mut expected = draw(app, theme, width);
    for pane in &mut app.panes {
        pane.entries[0].windows_attributes = Some(4);
    }
    if let Some(red) = red {
        recolour_names(&mut expected, app, red);
    }
    assert_eq!(
        draw(app, theme, width),
        expected,
        "{width}, {:?}",
        theme.level
    );
}

fn recolour_names(buffer: &mut Buffer, app: &App, red: Color) {
    for (id, area) in ui::geometry(app, buffer.area).panes.iter().enumerate() {
        let Some(area) = area else { continue };
        if app.panes[id].cursor.is_some() {
            continue;
        }
        let compact = area.width < 10;
        let marked = !app.panes[id].marks.is_empty();
        let start = area.x
            + if compact {
                1 + u16::from(marked) * 3
            } else {
                8
            };
        for x in start..area.right() - 1 {
            let cell = &mut buffer[(x, area.y + 3)];
            if cell.symbol() == " " {
                break;
            }
            cell.fg = red;
        }
    }
}

#[test]
fn cached_system_files_and_directories_colour_only_names_through_focus_marks_and_resize() {
    for directory in [false, true] {
        let (_root, mut app) = system_fixture(directory);
        for level in LEVELS {
            let red = expected_style(level, Some(Color::Red), false, false).fg;
            for state in 0..8 {
                set_state(&mut app, state);
                for active in 0..2 {
                    app.active = active;
                    for width in [240, 80, 7] {
                        compare_system_names(&mut app, &Theme::new(level), width, red);
                    }
                }
            }
        }
    }
}

#[test]
fn vte_system_names_keep_faint_effective_and_cursor_styles_unchanged() {
    let (_root, mut app) = system_fixture(false);
    let theme = Theme::with_vte_version(ColorLevel::TrueColor, Some("8400"));
    for state in 0..8 {
        set_state(&mut app, state);
        let red = if state & 1 != 0 {
            Color::Indexed(131)
        } else {
            Color::Rgb(190, 65, 70)
        };
        for active in 0..2 {
            app.active = active;
            compare_system_names(&mut app, &theme, 180, Some(red));
        }
    }
}
