//! Hidden flags occupy one type-column cell, independent of names and visibility.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
};
use std::{path::Path, sync::atomic::AtomicBool, time::Duration};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    fs::{self, EntryKind},
    input::{Bindings, Controller},
    ui,
    view::Columns,
};

fn loaded(root: &Path) -> App {
    let mut app = App::new(root.into());
    for id in 0..2 {
        app.complete_listing(
            id,
            1,
            Ok(fs::list(
                root,
                &fs::IgnoreContext::isolated(),
                &AtomicBool::new(false),
            )
            .unwrap()),
        );
        app.panes[id].columns = Columns {
            size: false,
            modified: false,
            attributes: false,
            owner: false,
        };
        app.panes[id].show_hidden = true;
        app.panes[id].refilter();
        app.panes[id].cursor = None;
    }
    app
}
fn fixture() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    for name in ["normal", "secret"] {
        std::fs::write(root.path().join(name), "data").unwrap();
    }
    let mut app = loaded(root.path());
    for pane in &mut app.panes {
        pane.entries
            .iter_mut()
            .find(|e| e.name == "secret")
            .unwrap()
            .hidden = true;
        pane.refilter();
        pane.cursor = None;
    }
    (root, app)
}
fn draw(app: &App, width: u16, level: ColorLevel) -> (Buffer, ui::Geometry) {
    let mut t = Terminal::new(TestBackend::new(width, 16)).unwrap();
    let mut g = None;
    t.draw(|f| g = Some(ui::render(f, app, &Theme::new(level))))
        .unwrap();
    (t.backend().buffer().clone(), g.unwrap())
}
fn entry_row(app: &App, id: usize, name: &str) -> u16 {
    app.panes[id]
        .visible
        .iter()
        .position(|i| app.panes[id].entries[*i].name == name)
        .unwrap() as u16
        + 2
}
#[test]
fn type_and_hidden_indicators_are_adjacent_for_each_existing_kind() {
    let (_root, mut app) = fixture();
    app.panes[0].entries.retain(|e| e.name == "normal");
    for (kind, indicator) in [
        (EntryKind::Directory, "d"),
        (EntryKind::File, "-"),
        (EntryKind::Link, "@"),
        (EntryKind::Junction, "j"),
        (EntryKind::ReparsePoint, "r"),
        (EntryKind::Socket, "s"),
        (EntryKind::Fifo, "p"),
        (EntryKind::Device, "D"),
        (EntryKind::Unknown, "?"),
    ] {
        for hidden in [false, true] {
            let entry = &mut app.panes[0].entries[0];
            entry.kind = kind;
            entry.hidden = hidden;
            entry.executable = false;
            entry.error = None;
            app.panes[0].refilter();
            app.panes[0].cursor = None;
            let (b, _) = draw(&app, 80, ColorLevel::NoColor);
            assert_eq!(b[(5, 2)].symbol(), indicator);
            assert_eq!(b[(6, 2)].symbol(), if hidden { "h" } else { " " });
            let name: String = (8..79).map(|x| b[(x, 2)].symbol()).collect();
            assert_eq!(
                name.trim_end(),
                if kind == EntryKind::Directory {
                    "normal/"
                } else {
                    "normal"
                }
            );
            assert_eq!(
                app.panes[0].measurements.name,
                if kind == EntryKind::Directory { 7 } else { 6 }
            );
        }
    }
}
#[test]
fn filenames_keep_their_column_across_visibility_focus_and_layout_changes() {
    let (_root, mut app) = fixture();
    for (width, dual) in [(12, false), (40, false), (160, true)] {
        app.dual = dual;
        for active in 0..2 {
            app.active = active;
            for shown in [true, false, true] {
                for pane in &mut app.panes {
                    pane.show_hidden = shown;
                    pane.refilter();
                    pane.cursor = None;
                }
                let (b, g) = draw(&app, width, ColorLevel::NoColor);
                for (id, area) in g.panes.iter().enumerate() {
                    if let Some(area) = area {
                        let row = entry_row(&app, id, "normal");
                        assert_eq!(b[(area.x + 6, row)].symbol(), " ");
                        assert_eq!(b[(area.x + 8, row)].symbol(), "n");
                        assert_eq!(app.panes[id].measurements.name, 6);
                        if shown {
                            let row = entry_row(&app, id, "secret");
                            assert_eq!(b[(area.x + 6, row)].symbol(), "h");
                            assert_eq!(b[(area.x + 8, row)].symbol(), "s");
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn hidden_accent_is_subtle_at_each_colour_level_and_ignore_dimming_stays_on_names() {
    let (_root, mut app) = fixture();
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let row = entry_row(&app, 0, "secret");
        let (plain, _) = draw(&app, 120, level);
        let h = &plain[(6, row)];
        assert_eq!(h.symbol(), "h");
        assert!(h.modifier.contains(Modifier::DIM));
        assert!(!h.modifier.contains(Modifier::BOLD));
        assert_eq!(h.fg == Color::Reset, level == ColorLevel::NoColor);
        if level == ColorLevel::Ansi16 {
            assert_eq!(h.fg, Color::Cyan);
        }
        let entry = app.panes[0]
            .entries
            .iter_mut()
            .find(|e| e.name == "secret")
            .unwrap();
        entry.ignored = Some(true);
        let (dimmed, _) = draw(&app, 120, level);
        assert_eq!(plain[(6, row)], dimmed[(6, row)]);
        assert_eq!(plain[(5, row)], dimmed[(5, row)]);
        assert!(!plain[(8, row)].modifier.contains(Modifier::DIM));
        assert!(dimmed[(8, row)].modifier.contains(Modifier::DIM));
        app.panes[0]
            .entries
            .iter_mut()
            .find(|e| e.name == "secret")
            .unwrap()
            .ignored = Some(false);
    }
}
#[test]
fn indicator_clicks_select_the_native_entry_without_toggling_preferences_or_marks() {
    let (_root, mut app) = fixture();
    app.dual = true;
    for pane in &mut app.panes {
        pane.select_all();
    }
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    for id in 0..2 {
        let (_, g) = draw(&c.app, 160, ColorLevel::NoColor);
        let x = g.panes[id].unwrap().x + 6;
        let row = entry_row(&c.app, id, "secret");
        let marks = c.app.panes[id].marks.clone();
        c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: x,
                row,
                modifiers: KeyModifiers::NONE,
            }),
            &g,
            Duration::ZERO,
        );
        assert_eq!(c.app.active, id);
        assert_eq!(c.app.panes[id].current().unwrap().name, "secret");
        assert_eq!(c.app.panes[id].marks, marks);
        assert!(!c.preferences_changed);
    }
}
#[cfg(unix)]
#[test]
fn native_unix_dot_files_directories_and_symlinks_use_the_hidden_cell() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(".file"), "data").unwrap();
    std::fs::create_dir(root.path().join(".dir")).unwrap();
    std::os::unix::fs::symlink(root.path().join(".dir"), root.path().join(".link")).unwrap();
    let app = loaded(root.path());
    let (b, _) = draw(&app, 120, ColorLevel::NoColor);
    for (name, kind, shown) in [
        (".file", "-", ".file"),
        (".dir", "d", ".dir/"),
        (".link", "@", ".link"),
    ] {
        let row = entry_row(&app, 0, name);
        assert_eq!(b[(5, row)].symbol(), kind);
        assert_eq!(b[(6, row)].symbol(), "h");
        let value: String = (8..119).map(|x| b[(x, row)].symbol()).collect();
        assert_eq!(value.trim_end(), shown);
        assert_eq!(
            app.panes[0]
                .entries
                .iter()
                .find(|e| e.name == name)
                .unwrap()
                .name,
            name
        );
    }
}
#[cfg(windows)]
#[test]
fn native_windows_hidden_attribute_drives_the_indicator_instead_of_dot_names() {
    let root = tempfile::tempdir().unwrap();
    for name in [".dotted", "secret"] {
        std::fs::write(root.path().join(name), "data").unwrap();
    }
    let status = std::process::Command::new("attrib")
        .arg("+h")
        .arg(root.path().join("secret"))
        .status()
        .unwrap();
    assert!(status.success());
    let app = loaded(root.path());
    let (b, _) = draw(&app, 120, ColorLevel::NoColor);
    for (name, indicator) in [(".dotted", " "), ("secret", "h")] {
        let row = entry_row(&app, 0, name);
        assert_eq!(b[(5, row)].symbol(), "-");
        assert_eq!(b[(6, row)].symbol(), indicator);
        assert_eq!(
            b[(8, row)].symbol(),
            if name == "secret" { "s" } else { "." }
        );
    }
}

#[test]
fn vte_hidden_markers_preserve_both_pane_cursors_marks_and_colour_levels() {
    let (root, mut app) = fixture();
    app.dual = true;
    for pane in &mut app.panes {
        pane.entries
            .iter_mut()
            .find(|e| e.name == "secret")
            .unwrap()
            .ignored = Some(true);
    }
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        for active in 0..2 {
            app.active = active;
            for marked in [false, true] {
                for cursor in ["normal", "secret"] {
                    for pane in &mut app.panes {
                        pane.marks.clear();
                        if marked {
                            pane.select_all();
                        }
                        pane.cursor = Some(root.path().join(cursor));
                    }
                    let theme = Theme::with_vte_version(level, Some("8400"));
                    let mut terminal = Terminal::new(TestBackend::new(180, 16)).unwrap();
                    terminal
                        .draw(|f| {
                            ui::render(f, &app, &theme);
                        })
                        .unwrap();
                    for id in 0..2 {
                        assert_vte_marker_cells(terminal.backend().buffer(), &app, id, &theme);
                    }
                }
            }
        }
    }
}

fn assert_vte_marker_cells(buffer: &Buffer, app: &App, id: usize, theme: &Theme) {
    let area = ui::geometry(app, buffer.area).panes[id].unwrap();
    let row = entry_row(app, id, "secret");
    let h = &buffer[(area.x + 6, row)];
    assert_eq!(h.symbol(), "h");
    if app.panes[id].current().unwrap().name == "secret" {
        let focus = theme.style(if id == app.active {
            Role::Cursor
        } else {
            Role::InactiveCursor
        });
        assert_eq!(Some(h.fg), focus.fg);
        assert_eq!(Some(h.bg), focus.bg);
        assert!(!h.modifier.contains(Modifier::DIM));
        assert_eq!(h.modifier.contains(Modifier::BOLD), id == app.active);
    } else {
        assert_eq!(h.modifier, Modifier::DIM);
        let expected = match theme.level {
            ColorLevel::NoColor => Color::Reset,
            ColorLevel::Ansi16 => Color::Cyan,
            _ => Color::Indexed(30),
        };
        assert_eq!(h.fg, expected);
        let normal = entry_row(app, id, "normal");
        assert_eq!(buffer[(area.x + 6, normal)].symbol(), " ");
    }
    assert_eq!(buffer[(area.x + 5, row)].symbol(), "-");
    assert_eq!(buffer[(area.x + 7, row)].symbol(), " ");
    assert_eq!(buffer[(area.x + 8, row)].symbol(), "s");
}
