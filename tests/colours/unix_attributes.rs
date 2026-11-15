//! Unix octal cells carry permission styling without a file-type prefix.
use super::*;
use terminal_file_manager::{permissions::PermissionDisplay, view::Columns};

#[test]
fn unix_octal_digits_keep_the_same_style_for_different_file_types() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("fixture"), "data").unwrap();
    let mut app = ignored_executable_app(root.path());
    for (kind, mode, octal) in [
        (EntryKind::Directory, 0o040775, "0775"),
        (EntryKind::File, 0o100664, "0664"),
        (EntryKind::Link, 0o127777, "7777"),
        (EntryKind::Device, 0o020000, "0000"),
        (EntryKind::Unknown, 0o177777, "7777"),
    ] {
        let pane = &mut app.panes[0];
        pane.marks.clear();
        pane.entries[0].kind = kind;
        pane.entries[0].mode = Some(mode);
        pane.entries[0].hidden = false;
        pane.entries[0].ignored = Some(false);
        pane.entries[0].executable = false;
        pane.permissions = PermissionDisplay::Octal;
        pane.columns = Columns {
            size: false,
            modified: false,
            attributes: true,
            owner: false,
        };
        pane.refilter();
        pane.cursor = None;
        for level in LEVELS {
            assert_octal_cells(&app, level, octal);
        }
    }
}

fn assert_octal_cells(app: &App, level: ColorLevel, octal: &str) {
    let theme = Theme::new(level);
    let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &theme);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let expected = expected_style(level, Some(Color::Yellow), false, false);
    for (offset, ch) in octal.chars().enumerate() {
        let cell = &buffer[(25 + offset as u16, 3)];
        assert_eq!(cell.symbol(), ch.to_string());
        assert_eq!(cell.fg, expected.fg.unwrap_or(Color::Reset));
        assert_eq!(cell.bg, buffer[(78, 3)].bg);
        assert_eq!(cell.modifier, expected.add_modifier);
    }
    assert_eq!(buffer[(29, 3)].symbol(), " ");
}
