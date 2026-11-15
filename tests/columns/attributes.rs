use super::*;
use ratatui::{buffer::Buffer, text::Line};
use terminal_file_manager::{
    input::{Action, Bindings, Controller},
    owner::OwnerDisplay,
    permissions::{PermissionDisplay, attributes},
};

fn formats() -> [(PermissionDisplay, &'static str, u16); 2] {
    if cfg!(windows) {
        [
            (PermissionDisplay::Symbolic, "Attr", 4),
            (PermissionDisplay::Octal, "Attributes", 10),
        ]
    } else {
        [
            (PermissionDisplay::Symbolic, "Attributes", 10),
            (PermissionDisplay::Octal, "Attr", 6),
        ]
    }
}

fn measure(entries: &[fs::Entry], display: PermissionDisplay) -> u16 {
    ui::Measurements::collect(entries.iter(), true, display, OwnerDisplay::default()).attributes
}

#[test]
fn attribute_heading_fits_while_the_first_snapshot_is_loading() {
    let root = tempfile::tempdir().unwrap();
    let app = App::new(root.path().into());
    assert!(row(&app, 100, 1).contains(symbolic_heading()));
}

#[test]
fn native_attribute_widths_include_headings_values_and_only_visible_unavailable_rows() {
    let (root, mut app) = loaded();
    for (display, heading, expected) in formats() {
        assert_eq!(measure(&[], display), Line::raw(heading).width() as u16);
        assert_eq!(measure(&app.panes[0].entries, display), expected);
        let missing = fs::inspect(&root.path().join("missing"));
        let pane = &mut app.panes[0];
        pane.permissions = display;
        pane.entries.push(missing);
        pane.set_filter(String::new());
        assert_eq!(pane.measurements.attributes, 11);
        pane.set_filter("界".into());
        assert_eq!(pane.measurements.attributes, expected);
        pane.entries.pop();
    }
}

#[test]
fn metadata_only_toggles_preserve_listing_heading_width_and_serialized_preferences() {
    for floating in [false, true] {
        let (_root, mut app) = loaded();
        app.dual = floating;
        let entry = app.panes[0].entries[0].clone();
        app.details[0].path = Some(entry.path.clone());
        app.details[0].entry = Some(entry);
        app.sidebar = !floating;
        app.panes[0].sidebar = !floating;
        app.popup = floating;
        let mut c = Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        );
        let saved = toml::to_string(&c.preferences).unwrap();
        let widths = c
            .app
            .panes
            .each_ref()
            .map(|pane| pane.measurements.attributes);
        let before = draw(&c.app, 240);
        let geometry = ui::geometry(&c.app, before.area);
        click_metadata_attributes(&mut c, &geometry);
        assert_eq!(
            c.app.metadata_format(floating).permissions,
            PermissionDisplay::Octal
        );
        assert_eq!(
            c.app
                .panes
                .each_ref()
                .map(|pane| pane.measurements.attributes),
            widths
        );
        assert_eq!(toml::to_string(&c.preferences).unwrap(), saved);
        let after = draw(&c.app, 240);
        assert_eq!(
            (0..240).map(|x| before[(x, 1)].clone()).collect::<Vec<_>>(),
            (0..240).map(|x| after[(x, 1)].clone()).collect::<Vec<_>>()
        );
        let text = after
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Attributes"));
    }
}

fn click_metadata_attributes(c: &mut Controller, geometry: &ui::Geometry) {
    use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let control = geometry
        .metadata_controls
        .iter()
        .find(|control| control.action == Action::Permissions)
        .unwrap();
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: control.area.x,
            row: control.area.y,
            modifiers: KeyModifiers::NONE,
        }),
        geometry,
        std::time::Duration::ZERO,
    );
}

#[test]
#[cfg(unix)]
fn opaque_unicode_attributes_use_display_cells_and_render_without_clipping() {
    let (_root, mut app) = loaded();
    let pane = &mut app.panes[0];
    let entry = &mut pane.entries[0];
    entry.mode = None;
    entry.windows_attributes = None;
    entry.attributes = "界界e\u{301}".into();
    pane.permissions = PermissionDisplay::Octal;
    pane.columns = Columns {
        size: false,
        modified: false,
        attributes: true,
        owner: true,
    };
    pane.refilter();
    let expected_width = 5.max(Line::raw(formats()[1].1).width() as u16);
    assert_eq!(pane.measurements.attributes, expected_width);
    let buffer = draw(&app, 80);
    let x = find(&buffer, 1, formats()[1].1);
    assert_eq!(buffer[(x, 3)].symbol(), "界");
    assert_eq!(buffer[(x + 2, 3)].symbol(), "界");
    assert_eq!(buffer[(x + 4, 3)].symbol(), "e\u{301}");
    assert_eq!(find(&buffer, 1, "Owner"), x + expected_width + 1);
}

fn draw(app: &App, width: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, 28)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

#[cfg(unix)]
fn find(buffer: &Buffer, y: u16, text: &str) -> u16 {
    (0..buffer.area.width)
        .find(|x| {
            text.chars().enumerate().all(|(i, ch)| {
                buffer
                    .cell((*x + i as u16, y))
                    .is_some_and(|cell| cell.symbol() == ch.to_string())
            })
        })
        .unwrap_or_else(|| panic!("missing {text:?} at row {y}"))
}

#[test]
fn toggling_each_pane_reflows_its_header_values_and_responsive_boundary() {
    let (_root, mut app) = loaded();
    app.dual = true;
    for pane in &mut app.panes {
        pane.columns = Columns {
            size: false,
            modified: false,
            attributes: true,
            owner: false,
        };
    }
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    for active in 0..2 {
        c.app.active = active;
        for (display, heading, expected) in [formats()[1], formats()[0]] {
            let other = c.app.panes[1 - active].measurements.attributes;
            c.dispatch(
                Action::Permissions,
                &ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 160, 28)),
            );
            let pane = &c.app.panes[active];
            assert_eq!(pane.permissions, display);
            assert_eq!(pane.measurements.attributes, expected);
            assert_eq!(c.app.panes[1 - active].measurements.attributes, other);
            assert_reflow(&c.app, heading, expected);
        }
    }
}

fn assert_reflow(app: &App, heading: &str, expected: u16) {
    let buffer = draw(app, 160);
    let geometry = ui::geometry(app, buffer.area);
    let pane = &app.panes[app.active];
    let area = geometry.panes[app.active].unwrap();
    let header = (area.x..area.right())
        .map(|x| buffer[(x, 1)].symbol())
        .collect::<String>();
    assert!(header.contains(heading));
    let value = attributes(&pane.entries[0], pane.permissions);
    let row = (area.x..area.right())
        .map(|x| buffer[(x, 3)].symbol())
        .collect::<String>();
    assert!(row.contains(&value));
    assert!(
        !pane
            .columns
            .visible(23 + expected, &pane.measurements)
            .attributes
    );
    assert!(
        pane.columns
            .visible(24 + expected, &pane.measurements)
            .attributes
    );
    for width in [7, 20, 25 + expected, 26 + expected] {
        let narrow = draw(app, width);
        let header = (0..width)
            .map(|x| narrow[(x, 1)].symbol())
            .collect::<String>();
        assert_eq!(header.contains(heading), width == 26 + expected);
        if width == 26 + expected {
            let row = (0..width)
                .map(|x| narrow[(x, 3)].symbol())
                .collect::<String>();
            assert!(row.contains(&value));
        }
    }
}
