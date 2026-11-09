use colored_text::ColorLevel;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Modifier};
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Bindings, Controller},
    ui,
};

fn send(c: &mut Controller, code: KeyCode, width: u16) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, width, 24));
    c.handle(
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        &g,
        Default::default(),
    );
}

#[test]
fn every_stage_preserves_geometry_scroll_and_status_across_widths_and_colours() {
    let root = tempfile::tempdir().unwrap();
    for width in [0, 1, 2, 8, 24, 40, 80, 120, 160] {
        for dual in [false, true] {
            for maximized in [false, true] {
                lifecycle(root.path(), width, dual, maximized);
            }
        }
    }
}

fn assert_selected_style(c: &Controller, g: &ui::Geometry, width: u16, level: ColorLevel) {
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, &c.app, &Theme::new(level));
        })
        .unwrap();
    if let Some(layout) = &g.workflow {
        for control in &layout.controls {
            assert!(g.footer.contains((control.area.x, control.area.y).into()));
            if control.selected {
                assert!(control.label.starts_with('['));
                assert!(
                    terminal.backend().buffer()[(control.area.x, control.area.y)]
                        .modifier
                        .contains(Modifier::BOLD | Modifier::REVERSED)
                );
            }
        }
    }
}

fn lifecycle(root: &std::path::Path, width: u16, dual: bool, maximized: bool) {
    let mut app = App::new(root.into());
    app.launch_directory = Some(root.into());
    app.dual = dual;
    app.maximized = maximized;
    app.sidebar = true;
    app.panes[0].scroll = 7;
    app.panes[1].scroll = 9;
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );

    let status = c.app.status.clone();
    let baseline = ui::geometry(&c.app, Rect::new(0, 0, width, 24));
    for key in [
        KeyCode::Char('y'),
        KeyCode::Enter,
        KeyCode::Left,
        KeyCode::Enter,
        KeyCode::Esc,
    ] {
        send(&mut c, key, width);
        let g = ui::geometry(&c.app, Rect::new(0, 0, width, 24));
        assert_eq!(
            (g.panes, g.divider, g.sidebar, g.status, g.footer),
            (
                baseline.panes,
                baseline.divider,
                baseline.sidebar,
                baseline.status,
                baseline.footer
            )
        );
        assert_eq!((c.app.panes[0].scroll, c.app.panes[1].scroll), (7, 9));
        for level in [
            ColorLevel::NoColor,
            ColorLevel::Ansi16,
            ColorLevel::Ansi256,
            ColorLevel::TrueColor,
        ] {
            assert_selected_style(&c, &g, width, level);
        }
    }
    assert!(c.app.footer_workflow.is_none());
    assert_eq!(c.app.status, status);
}
