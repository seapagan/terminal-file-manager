//! The directory picker uses the existing contextual Help layer and title controls.
use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect, style::Color};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Action, Bindings, Controller, HelpTopic, chord},
    ui,
};

fn controller(long: bool) -> Controller {
    let path = std::path::PathBuf::from(if cfg!(windows) {
        r"C:\tfm-fixture"
    } else {
        "/tfm-fixture"
    })
    .join("current-directory");
    let path = if long {
        (0..24).fold(path, |path, _| path.join("ancestor"))
    } else {
        path
    };
    Controller::new(
        App::new(path),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}

fn event(c: &mut Controller, event: Event, bounds: Rect) {
    let g = ui::geometry(&c.app, bounds);
    assert!(c.handle(event, &g, Default::default()).is_empty());
}

fn key(c: &mut Controller, name: &str, bounds: Rect) {
    let (code, modifiers) = chord(name).unwrap();
    event(c, Event::Key(KeyEvent::new(code, modifiers)), bounds);
}

fn click(c: &mut Controller, area: Rect, bounds: Rect) {
    event(
        c,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }),
        bounds,
    );
}

fn snapshot(c: &Controller) -> (usize, std::path::PathBuf, usize, usize) {
    let state = c.app.path_picker.as_ref().unwrap();
    (state.pane, state.path.clone(), state.selected, state.scroll)
}

fn draw(c: &Controller, bounds: Rect, theme: &Theme) -> (Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    let mut geometry = ui::geometry(&c.app, bounds);
    terminal
        .draw(|frame| geometry = ui::render(frame, &c.app, theme))
        .unwrap();
    (terminal.backend().buffer().clone(), geometry)
}

#[test]
fn both_help_keys_capture_input_and_restore_picker_geometry_selection_and_scroll() {
    let bounds = Rect::new(0, 0, 80, 24);
    for help in ["?", "F1"] {
        let mut c = controller(true);
        key(&mut c, "Alt+l", bounds);
        let before = snapshot(&c);
        let original = ui::geometry(&c.app, bounds).path_picker.unwrap();
        key(&mut c, help, bounds);
        assert!(c.app.context_help.is_some());
        assert_eq!(
            c.app.context_help.as_ref().unwrap().topic,
            HelpTopic::FullPath
        );
        for blocked in ["Enter", "Down", "Tab", "F8", "q", "Alt+l"] {
            key(&mut c, blocked, bounds);
        }
        event(&mut c, Event::Paste("ignored".into()), bounds);
        click(&mut c, original.controls[0].0.area, bounds);
        assert_eq!(snapshot(&c), before);
        assert!(c.app.operations.dialog.is_none() && c.app.operations.outbox.is_empty());
        assert!(!c.quit && c.app.panes[0].pending.is_none());
        key(&mut c, "Esc", bounds);
        assert!(c.app.context_help.is_none());
        assert_eq!(snapshot(&c), before);
        let restored = ui::geometry(&c.app, bounds).path_picker.unwrap();
        assert_eq!(original.shell.outer, restored.shell.outer);
        assert_eq!(original.shell.body, restored.shell.body);
        key(&mut c, "Esc", bounds);
        assert!(c.app.path_picker.is_none());
    }
}

#[test]
fn title_help_button_uses_shared_rendered_cells_and_click_bounds_for_both_opening_routes() {
    let bounds = Rect::new(0, 0, 80, 24);
    for mouse in [false, true] {
        for level in [
            ColorLevel::NoColor,
            ColorLevel::Ansi16,
            ColorLevel::Ansi256,
            ColorLevel::TrueColor,
        ] {
            let mut c = controller(false);
            if mouse {
                let g = ui::geometry(&c.app, Rect::new(0, 0, 24, 24));
                let area = g
                    .path_controls
                    .iter()
                    .find(|control| control.target.is_none())
                    .unwrap()
                    .area;
                click(&mut c, area, Rect::new(0, 0, 24, 24));
            } else {
                key(&mut c, "Alt+l", bounds);
            }
            let theme = Theme::new(level);
            let (buffer, g) = draw(&c, bounds, &theme);
            let shell = &g.path_picker.as_ref().unwrap().shell;
            let help = shell.help.expect("picker title has context Help");
            assert_eq!(Some(help), g.context_control);
            assert!(!help.intersects(shell.close.unwrap()));
            for (offset, symbol) in ["[", "?", "]"].iter().enumerate() {
                let cell = &buffer[(help.x + offset as u16, help.y)];
                assert_eq!(cell.symbol(), *symbol);
                assert_eq!(cell.fg, theme.style(Role::Key).fg.unwrap_or(Color::Reset));
                assert_eq!(cell.modifier, theme.style(Role::Key).add_modifier);
            }
            click(&mut c, help, bounds);
            assert!(c.app.context_help.is_some());
            let back = ui::geometry(&c.app, bounds)
                .context_help
                .unwrap()
                .shell
                .actions[0]
                .area;
            click(&mut c, back, bounds);
            assert!(c.app.context_help.is_none() && c.app.path_picker.is_some());
        }
    }
}

#[test]
fn narrow_picker_keeps_f1_access_when_title_controls_do_not_fit() {
    for width in [1, 12, 20, 24, 40, 80, 120] {
        for height in [6, 24] {
            let bounds = Rect::new(0, 0, width, height);
            let mut c = controller(true);
            key(&mut c, "Alt+l", bounds);
            let original = snapshot(&c);
            if width < 28 {
                assert!(
                    ui::geometry(&c.app, bounds)
                        .path_picker
                        .unwrap()
                        .shell
                        .help
                        .is_none()
                );
            }
            key(&mut c, "F1", bounds);
            assert!(c.app.context_help.is_some());
            draw(&c, bounds, &Theme::new(ColorLevel::NoColor));
            key(&mut c, "Esc", bounds);
            assert_eq!(snapshot(&c), original);
            assert!(c.app.context_help.is_none());
        }
    }
}

#[test]
fn help_prose_uses_the_effective_binding_and_both_close_buttons_remain_safe() {
    let bindings = Bindings::new(&std::collections::BTreeMap::from([
        ("F2".into(), Action::FullPath),
        ("Alt+l".into(), Action::Refresh),
        ("Esc".into(), Action::Delete),
        ("?".into(), Action::Delete),
    ]))
    .unwrap();
    let mut c = controller(false);
    c = Controller::new(c.app, bindings, Default::default());
    let bounds = Rect::new(0, 0, 100, 40);
    key(&mut c, "F2", bounds);
    key(&mut c, "?", bounds);
    assert!(c.app.context_help.is_none());
    key(&mut c, "F1", bounds);
    let (buffer, g) = draw(&c, bounds, &Theme::new(ColorLevel::NoColor));
    let text = buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("Choose a parent directory"));
    assert!(text.contains("Open from the file list: F2."));
    assert!(!text.contains("Alt+l"));
    let help_close = g.context_help.as_ref().unwrap().shell.close.unwrap();
    assert_eq!(g.modal_close, Some(help_close));
    click(&mut c, help_close, bounds);
    assert!(c.app.context_help.is_none() && c.app.path_picker.is_some());
    let parent_close = ui::geometry(&c.app, bounds)
        .path_picker
        .unwrap()
        .shell
        .close
        .unwrap();
    click(&mut c, parent_close, bounds);
    assert!(c.app.path_picker.is_none());
    assert!(c.app.operations.dialog.is_none() && c.app.operations.outbox.is_empty());
}

#[test]
fn resizing_help_reveals_the_parent_selection_and_returns_to_ancestor_navigation() {
    let mut c = controller(true);
    let mut bounds = Rect::new(0, 0, 80, 24);
    key(&mut c, "Alt+l", bounds);
    let original = snapshot(&c);
    for (width, height) in [(24, 12), (80, 24), (40, 12), (120, 24)] {
        key(&mut c, "F1", bounds);
        event(&mut c, Event::Resize(width, height), bounds);
        bounds = Rect::new(0, 0, width, height);
        let (_, g) = draw(&c, bounds, &Theme::new(ColorLevel::NoColor));
        assert!(
            c.app.context_help.as_ref().unwrap().scroll <= g.context_help.unwrap().scroll_limit
        );
        key(&mut c, "Esc", bounds);
        let state = snapshot(&c);
        assert_eq!(
            (&state.0, &state.1, &state.2),
            (&original.0, &original.1, &original.2)
        );
        let layout = ui::geometry(&c.app, bounds).path_picker.unwrap();
        assert!(layout.controls.iter().any(|(_, index)| *index == state.2));
        assert!(state.3 <= layout.limit);
    }
    key(&mut c, "Left", bounds);
    key(&mut c, "F1", bounds);
    key(&mut c, "Esc", bounds);
    let g = ui::geometry(&c.app, bounds);
    let requests = c.handle(
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &g,
        Default::default(),
    );
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, original.1.parent().unwrap());
    assert!(c.app.path_picker.is_none());
}

#[test]
fn help_ignores_nonpress_opening_events_and_interrupts_unwind_both_layers() {
    let bounds = Rect::new(0, 0, 80, 24);
    let mut c = controller(false);
    key(&mut c, "Alt+l", bounds);
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        event(
            &mut c,
            Event::Key(KeyEvent::new_with_kind(
                KeyCode::F(1),
                KeyModifiers::NONE,
                kind,
            )),
            bounds,
        );
        assert!(c.app.context_help.is_none());
    }
    key(&mut c, "F1", bounds);
    key(&mut c, "Ctrl+c", bounds);
    assert!(c.app.context_help.is_none() && c.app.path_picker.is_none());
    assert!(c.quit);
    assert!(c.app.operations.dialog.is_none() && c.app.operations.outbox.is_empty());
}
