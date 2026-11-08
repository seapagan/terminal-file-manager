//! Both picker entry points size to content and wrap only at terminal bounds.
use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect, text::Line};
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Bindings, Controller},
    ui,
};

fn path() -> std::path::PathBuf {
    std::path::PathBuf::from(if cfg!(windows) {
        r"C:\tfm-fixture"
    } else {
        "/tfm-fixture"
    })
    .join("ancestor")
    .join("界e\u{301}")
    .join("current-directory")
}

fn opened(path: std::path::PathBuf, mouse: bool, width: u16) -> Controller {
    let mut c = Controller::new(
        App::new(path),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let g = ui::geometry(&c.app, Rect::new(0, 0, if mouse { 40 } else { width }, 24));
    let event = if mouse {
        let control = g
            .path_controls
            .iter()
            .find(|control| control.target.is_none())
            .unwrap();
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: control.area.x,
            row: control.area.y,
            modifiers: KeyModifiers::NONE,
        })
    } else {
        Event::Key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT))
    };
    assert!(c.handle(event, &g, Default::default()).is_empty());
    if mouse {
        c.handle(Event::Resize(width, 24), &g, Default::default());
    }
    c
}

fn draw(c: &Controller, width: u16) -> (Buffer, ui::PathLayout) {
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
    let mut geometry = ui::geometry(&c.app, Rect::new(0, 0, width, 24));
    terminal
        .draw(|frame| geometry = ui::render(frame, &c.app, &Theme::new(ColorLevel::NoColor)))
        .unwrap();
    (
        terminal.backend().buffer().clone(),
        geometry.path_picker.unwrap(),
    )
}

#[test]
fn short_picker_fits_native_display_cells_with_small_padding_for_both_opening_routes() {
    let text = ui::clean(&path().to_string_lossy());
    let expected_width = Line::raw(&text).width() as u16 + 4;
    let expected = Buffer::with_lines([text.as_str()]);
    let expected = expected
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    let mut previous = None;
    for mouse in [false, true] {
        for width in [120, 240, 400] {
            let c = opened(path(), mouse, width);
            let (buffer, layout) = draw(&c, width);
            assert_eq!(layout.shell.outer.width, expected_width);
            assert_eq!(layout.shell.body.width, expected_width - 4);
            assert_eq!(layout.shell.body.height, 1);
            assert_eq!(layout.shell.outer.x, (width - expected_width) / 2);
            assert_eq!(layout.scroll, 0);
            let rendered = (layout.shell.body.x..layout.shell.body.right())
                .map(|x| buffer[(x, layout.shell.body.y)].symbol())
                .collect::<String>();
            assert_eq!(rendered.trim_end(), expected);
            assert!(
                layout.controls.iter().all(|(control, _)| layout
                    .shell
                    .body
                    .intersection(control.area)
                    == control.area)
            );
            let shape = (layout.shell.outer.width, layout.shell.outer.height);
            if let Some(previous) = previous {
                assert_eq!(shape, previous);
            }
            previous = Some(shape);
        }
    }
}

#[test]
fn root_picker_keeps_complete_hints_without_filling_the_terminal() {
    let c = opened(path().ancestors().last().unwrap().to_path_buf(), false, 120);
    let (buffer, layout) = draw(&c, 120);
    assert!(layout.shell.outer.width < 40);
    let footer = layout.shell.footer;
    let text = (footer.x..footer.right())
        .map(|x| buffer[(x, footer.y)].symbol())
        .collect::<String>();
    assert_eq!(text.trim(), "Arrows select · Enter open");
    assert_eq!(layout.shell.body.height, 1);
}

#[test]
fn oversized_picker_caps_width_and_wraps_the_same_content_after_resizing() {
    let path = path().join("long-segment-".repeat(40));
    for mouse in [false, true] {
        let mut c = opened(path.clone(), mouse, 120);
        for width in [120, 40, 80, 240] {
            let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
            c.handle(Event::Resize(width, 24), &g, Default::default());
            let (_, layout) = draw(&c, width);
            assert_eq!(layout.shell.outer.width, width - 2);
            assert!(
                layout
                    .controls
                    .iter()
                    .any(|(control, _)| control.area.y > layout.shell.body.y)
            );
            let selected = c.app.path_picker.as_ref().unwrap().selected;
            assert!(layout.controls.iter().any(|(_, index)| *index == selected));
            assert!(
                layout.controls.iter().all(|(control, _)| layout
                    .shell
                    .body
                    .intersection(control.area)
                    == control.area)
            );
        }
    }
}
