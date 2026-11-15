//! Windows attribute symbols stay separate from type and System filename accents.
use super::*;
use terminal_file_manager::{permissions::PermissionDisplay, view::Columns};

const FLAGS: [(u32, &str); 16] = [
    (0, "----"),
    (1, "R---"),
    (2, "-H--"),
    (3, "RH--"),
    (4, "--S-"),
    (5, "R-S-"),
    (6, "-HS-"),
    (7, "RHS-"),
    (32, "---A"),
    (33, "R--A"),
    (34, "-H-A"),
    (35, "RH-A"),
    (36, "--SA"),
    (37, "R-SA"),
    (38, "-HSA"),
    (39, "RHSA"),
];

fn prepare_attributes(app: &mut App, flags: u32, state: u8, display: PermissionDisplay) {
    for pane in &mut app.panes {
        pane.entries[0].windows_attributes = Some(flags);
        pane.entries[0].hidden = flags & 2 != 0;
        pane.permissions = display;
        pane.columns = Columns {
            size: false,
            modified: false,
            attributes: true,
            owner: false,
        };
        pane.refilter();
    }
    // Keep ignored-name and mark cues present while exercising both cursor states.
    set_state(app, 5 | (state & 2));
    app.active = usize::from(state & 1);
}

#[test]
fn windows_symbolic_flags_use_normal_or_muted_cells_with_cursor_precedence() {
    for directory in [false, true] {
        let (_root, mut app) = system_fixture(directory);
        for level in LEVELS {
            for state in 0..4 {
                for (flags, symbolic) in FLAGS {
                    prepare_attributes(&mut app, flags, state, PermissionDisplay::Symbolic);
                    let buffer = draw(&app, &Theme::new(level), 240);
                    assert_attribute_cells(&buffer, &app, level, flags, symbolic);
                }
            }
        }
    }
}

#[test]
fn windows_hexadecimal_cells_keep_type_warning_and_cursor_styles() {
    for directory in [false, true] {
        let (_root, mut app) = system_fixture(directory);
        for level in LEVELS {
            for state in 0..4 {
                prepare_attributes(&mut app, 4, state, PermissionDisplay::Octal);
                let buffer = draw(&app, &Theme::new(level), 240);
                assert_hex_cells(&buffer, &app, level, directory);
            }
        }
    }
}

fn assert_hex_cells(buffer: &Buffer, app: &App, level: ColorLevel, directory: bool) {
    for (id, area) in ui::geometry(app, buffer.area).panes.iter().enumerate() {
        let area = area.unwrap();
        let (x, y) = (area.x + 25, area.y + 3);
        let normal = &buffer[(area.right() - 2, y)];
        let pane = &app.panes[id];
        assert_eq!(pane.measurements.attributes, 10);
        for (offset, symbol) in pane.entries[0].native_attributes().chars().enumerate() {
            let mut expected = normal.clone();
            expected.set_symbol(&symbol.to_string());
            if pane.cursor.is_none() {
                let colour = if offset != 0 {
                    Color::Yellow
                } else if directory {
                    Color::Blue
                } else {
                    Color::Green
                };
                let style = expected_style(level, Some(colour), offset == 0 && !directory, false);
                expected.fg = style.fg.unwrap_or(normal.fg);
                expected.modifier |= style.add_modifier;
            }
            assert_eq!(buffer[(x + offset as u16, y)], expected);
        }
        let heading = (x..x + 10)
            .map(|x| buffer[(x, area.y + 1)].symbol())
            .collect::<String>();
        assert_eq!(heading, "Attributes");
    }
}

fn assert_attribute_cells(
    buffer: &Buffer,
    app: &App,
    level: ColorLevel,
    flags: u32,
    symbolic: &str,
) {
    for (id, area) in ui::geometry(app, buffer.area).panes.iter().enumerate() {
        let area = area.unwrap();
        // Marks/type/name consume 24 inner cells, including existing spacing.
        let (x, y) = (area.x + 25, area.y + 3);
        let normal = &buffer[(area.right() - 2, y)];
        let cursor = app.panes[id].cursor.is_some();
        for (offset, symbol) in symbolic.chars().enumerate() {
            let cell = &buffer[(x + offset as u16, y)];
            assert_eq!(cell.symbol(), symbol.to_string());
            let mut expected = normal.clone();
            expected.set_symbol(&symbol.to_string());
            if symbol == '-' && !cursor {
                expected.modifier.insert(Modifier::DIM);
                if level != ColorLevel::NoColor {
                    expected.fg = Color::DarkGray;
                }
            }
            assert_eq!(cell, &expected, "{level:?} {flags:#x} pane {id}");
        }
        let heading = (x..x + 4)
            .map(|x| buffer[(x, area.y + 1)].symbol())
            .collect::<String>();
        assert_eq!(heading, "Attr");
        assert_eq!(buffer[(x + 4, y)].symbol(), " ");
        assert_eq!(buffer[(area.x + 2, y)].symbol(), "*");
        if flags & 4 != 0 && !cursor && level != ColorLevel::NoColor {
            let red = expected_style(level, Some(Color::Red), false, false)
                .fg
                .unwrap();
            assert_eq!(buffer[(area.x + 8, y)].fg, red);
        }
    }
}
