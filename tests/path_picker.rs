//! Picker input, hover and overlay ownership use rendered controls.
use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Action, Bindings, Controller},
    navigation::PathPicker,
    settings::Settings,
    ui,
};

fn controller() -> Controller {
    let path = std::env::temp_dir().join("parent".repeat(30)).join("leaf");
    let mut app = App::new(path.clone());
    app.path_picker = Some(PathPicker {
        pane: 0,
        path,
        selected: 0,
        scroll: 0,
    });
    Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    )
}

fn geometry(c: &Controller) -> ui::Geometry {
    ui::geometry(&c.app, Rect::new(0, 0, 80, 24))
}

fn click(x: u16, y: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn picker_mouse_and_keyboard_select_the_same_native_ancestor() {
    for keyboard in [false, true] {
        let mut c = controller();
        let g = geometry(&c);
        let layout = g.path_picker.as_ref().unwrap();
        let control = &layout.controls[0].0;
        let expected = control.target.clone().unwrap();
        let event = if keyboard {
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        } else {
            click(control.area.x, control.area.y)
        };
        let requests = c.handle(event, &g, Default::default());
        assert_eq!(requests[0].path, expected);
        assert!(c.app.path_picker.is_none());
        assert_eq!(c.app.active, 0);
    }
}

#[test]
fn title_close_and_cancel_close_without_navigation_and_modifiers_are_inert() {
    for title in [false, true] {
        let mut c = controller();
        let g = geometry(&c);
        let original = c.app.panes[0].path.clone();
        for modifiers in [
            KeyModifiers::ALT,
            KeyModifiers::CONTROL,
            KeyModifiers::SHIFT,
        ] {
            assert!(
                c.handle(
                    Event::Key(KeyEvent::new(KeyCode::Enter, modifiers)),
                    &g,
                    Default::default()
                )
                .is_empty()
            );
            assert!(c.app.path_picker.is_some());
            assert!(c.app.panes[0].pending.is_none());
        }
        let layout = g.path_picker.as_ref().unwrap();
        let area = if title {
            layout.shell.close.unwrap()
        } else {
            layout.shell.actions[0].area
        };
        assert!(
            c.handle(click(area.x, area.y), &g, Default::default())
                .is_empty()
        );
        assert!(c.app.path_picker.is_none());
        assert_eq!(c.app.panes[0].path, original);
    }
}

#[test]
fn modal_capture_blocks_file_selection_operations_and_respects_safety_interrupts() {
    let mut c = controller();
    let g = geometry(&c);
    let before = c.app.panes[0].cursor.clone();
    for event in [
        click(8, 2),
        Event::Paste("q".into()),
        Event::Key(KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE)),
    ] {
        assert!(c.handle(event, &g, Default::default()).is_empty());
        assert_eq!(c.app.panes[0].cursor, before);
        assert!(c.app.operations.outbox.is_empty());
        assert!(c.app.operations.dialog.is_none());
    }
    assert!(c.dispatch(Action::Copy, &g).is_empty());
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        &g,
        Default::default(),
    );
    assert!(c.quit);
    assert!(c.app.path_picker.is_none());
}

#[test]
fn modal_hover_styles_every_wrapped_fragment_with_the_semantic_role() {
    let mut c = controller();
    let g = geometry(&c);
    let layout = g.path_picker.as_ref().unwrap();
    let index = layout.controls.last().unwrap().1;
    let area = layout
        .controls
        .iter()
        .find(|(_, i)| *i == index)
        .unwrap()
        .0
        .area;
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let theme = Theme::new(level);
        c.app.path_pointer = Some((area.x, area.y));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|f| {
                ui::render(f, &c.app, &theme);
            })
            .unwrap();
        for (control, _) in layout.controls.iter().filter(|(_, i)| *i == index) {
            let cell = &terminal.backend().buffer()[(control.area.x, control.area.y)];
            let style = theme.style(Role::PathHover);
            assert_eq!(cell.fg, style.fg.unwrap_or(ratatui::style::Color::Reset));
            assert_eq!(cell.bg, ratatui::style::Color::Reset);
            assert_eq!(cell.modifier, style.add_modifier);
        }
    }
}

#[test]
fn modified_tab_does_not_move_picker_focus() {
    let mut c = controller();
    let g = geometry(&c);
    let before = c.app.path_picker.as_ref().unwrap().selected;
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL)),
        &g,
        Default::default(),
    );
    assert_eq!(c.app.path_picker.as_ref().unwrap().selected, before);
}
