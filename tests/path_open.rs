//! Keyboard and ellipsis openings share native selection and measured reveal.
use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Action, Bindings, Controller, HelpTarget, help_hits},
    navigation::path_segments,
    settings::Settings,
    ui,
};

fn path() -> std::path::PathBuf {
    std::path::PathBuf::from(if cfg!(windows) {
        r"C:\tfm-fixture"
    } else {
        "/tfm-fixture"
    })
    .join("leaf")
}

#[test]
fn full_path_default_binding_is_separate_from_reserved_manual_path_entry() {
    let bindings = Bindings::new(&Default::default()).unwrap();
    assert_eq!(
        bindings.action(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT)),
        Some(Action::FullPath)
    );
    assert_eq!(
        bindings.action(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)),
        Some(Action::Path)
    );
}

#[test]
fn remapped_full_path_binding_opens_the_picker_and_appears_in_help_search() {
    let settings = Settings::parse(
        r#"[bindings]
"Alt+l" = "refresh"
F2 = "full-path"
"#,
    )
    .unwrap();
    let bindings = Bindings::new(&settings.bindings).unwrap();
    assert_eq!(
        bindings.action(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT)),
        Some(Action::Refresh)
    );
    let entries = bindings.help_entries();
    let entry = entries
        .iter()
        .find(|entry| entry.action == Action::FullPath)
        .unwrap();
    assert_eq!(entry.keys, "F2");
    for query in ["full path", "fullpath", "F2"] {
        assert!(
            help_hits(&entries, query)
                .iter()
                .any(|hit| hit.target == HelpTarget::Action(Action::FullPath))
        );
    }
    let mut c = Controller::new(App::new(path()), bindings, settings);
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    c.handle(
        key(KeyCode::F(2), KeyModifiers::NONE),
        &g,
        Default::default(),
    );
    assert!(c.app.path_picker.is_some());
    c.handle(
        key(KeyCode::Esc, KeyModifiers::NONE),
        &g,
        Default::default(),
    );
    c.handle(
        key(KeyCode::Char('l'), KeyModifiers::ALT),
        &g,
        Default::default(),
    );
    assert!(c.app.path_picker.is_none());
}

#[test]
fn opened_picker_cancels_or_navigates_to_the_chosen_ancestor() {
    for cancel in [false, true] {
        let mut c = controller(path());
        let area = Rect::new(0, 0, 80, 24);
        let g = ui::geometry(&c.app, area);
        c.handle(
            key(KeyCode::Char('l'), KeyModifiers::ALT),
            &g,
            Default::default(),
        );
        let g = ui::geometry(&c.app, area);
        c.handle(
            key(KeyCode::Left, KeyModifiers::NONE),
            &g,
            Default::default(),
        );
        let requests = c.handle(
            key(
                if cancel { KeyCode::Esc } else { KeyCode::Enter },
                KeyModifiers::NONE,
            ),
            &g,
            Default::default(),
        );
        assert!(c.app.path_picker.is_none());
        if cancel {
            assert!(requests.is_empty());
            assert_eq!(c.app.panes[0].path, path());
        } else {
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].path, path().parent().unwrap());
        }
    }
}

#[test]
fn picker_opening_respects_press_events_help_capture_and_modal_input_isolation() {
    use crossterm::event::KeyEventKind;
    let mut c = controller(path());
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        c.handle(
            Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char('l'),
                KeyModifiers::ALT,
                kind,
            )),
            &g,
            Default::default(),
        );
        assert!(c.app.path_picker.is_none());
    }
    c.dispatch(Action::Help, &g);
    c.handle(
        key(KeyCode::Char('l'), KeyModifiers::ALT),
        &g,
        Default::default(),
    );
    assert!(c.app.path_picker.is_none());
    c.handle(
        key(KeyCode::Esc, KeyModifiers::NONE),
        &g,
        Default::default(),
    );
    c.handle(
        key(KeyCode::Char('l'), KeyModifiers::ALT),
        &g,
        Default::default(),
    );
    let selected = c.app.path_picker.as_ref().unwrap().selected;
    for event in [
        key(KeyCode::Char('l'), KeyModifiers::ALT),
        key(KeyCode::F(8), KeyModifiers::NONE),
        key(KeyCode::Char('q'), KeyModifiers::NONE),
        Event::Paste("ignored".into()),
    ] {
        assert!(c.handle(event, &g, Default::default()).is_empty());
        assert_eq!(c.app.path_picker.as_ref().unwrap().selected, selected);
        assert!(!c.quit);
        assert!(c.app.operations.dialog.is_none());
    }
}

#[test]
fn opening_reveals_selection_using_the_original_offset_geometry() {
    let path = (0..30).fold(path(), |path, _| path.join("wrapped-ancestor"));
    let mut c = controller(path);
    let area = Rect::new(7, 5, 40, 12);
    let g = ui::geometry(&c.app, area);
    c.handle(
        key(KeyCode::Char('l'), KeyModifiers::ALT),
        &g,
        Default::default(),
    );
    let state = c.app.path_picker.as_ref().unwrap();
    assert!(state.scroll > 0);
    let g = ui::geometry(&c.app, area);
    let layout = g.path_picker.unwrap();
    assert_eq!(area.intersection(layout.shell.outer), layout.shell.outer);
    assert!(
        layout
            .controls
            .iter()
            .any(|(_, index)| *index == state.selected)
    );
}

#[test]
fn immediate_enter_closes_picker_without_navigation_even_without_visible_controls() {
    for area in [Rect::new(0, 0, 1, 1), Rect::new(0, 0, 80, 24)] {
        let mut c = controller(path());
        let g = ui::geometry(&c.app, area);
        c.handle(
            key(KeyCode::Char('l'), KeyModifiers::ALT),
            &g,
            Default::default(),
        );
        assert!(c.app.path_picker.is_some());
        assert!(
            c.handle(
                key(KeyCode::Enter, KeyModifiers::NONE),
                &g,
                Default::default()
            )
            .is_empty()
        );
        assert!(c.app.path_picker.is_none());
        assert!(c.app.panes[0].pending.is_none());
        assert_eq!(c.app.panes[0].path, path());
    }
}

fn controller(path: std::path::PathBuf) -> Controller {
    Controller::new(
        App::new(path),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(code, modifiers))
}

#[test]
fn alt_l_opens_untruncated_paths_and_roots_in_the_active_pane_and_enter_stays_put() {
    for path in [path(), path().ancestors().last().unwrap().to_path_buf()] {
        for active in 0..2 {
            let mut c = controller(path.clone());
            c.app.dual = true;
            c.app.active = active;
            let g = ui::geometry(&c.app, Rect::new(0, 0, 240, 24));
            assert!(
                g.path_controls
                    .iter()
                    .all(|control| control.target.is_some())
            );
            assert!(
                c.handle(
                    key(KeyCode::Char('l'), KeyModifiers::ALT),
                    &g,
                    Default::default()
                )
                .is_empty()
            );
            let state = c.app.path_picker.as_ref().expect("Alt+L opens picker");
            assert_eq!(
                (state.pane, state.selected),
                (active, path_segments(&path).len() - 1)
            );
            assert_eq!(state.path, path);
            let g = ui::geometry(&c.app, Rect::new(0, 0, 240, 24));
            assert!(
                c.handle(
                    key(KeyCode::Enter, KeyModifiers::NONE),
                    &g,
                    Default::default()
                )
                .is_empty()
            );
            assert!(c.app.path_picker.is_none());
            assert_eq!(c.app.panes[active].path, path);
            assert!(c.app.panes[active].pending.is_none());
        }
    }
}

#[test]
fn both_opening_routes_select_and_render_the_current_wrapped_segment() {
    let path = (0..24)
        .fold(path(), |path, index| path.join(format!("ancestor-{index}")))
        .join("界e\u{301}".repeat(80));
    for mouse in [false, true] {
        for level in [
            ColorLevel::NoColor,
            ColorLevel::Ansi16,
            ColorLevel::Ansi256,
            ColorLevel::TrueColor,
        ] {
            let mut c = controller(path.clone());
            c.app.dual = true;
            c.app.active = usize::from(!mouse);
            let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 12));
            let event = if mouse {
                let control = g
                    .path_controls
                    .iter()
                    .find(|control| control.pane == 1 && control.target.is_none())
                    .unwrap();
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: control.area.x,
                    row: control.area.y,
                    modifiers: KeyModifiers::NONE,
                })
            } else {
                key(KeyCode::Char('l'), KeyModifiers::ALT)
            };
            assert!(c.handle(event, &g, Default::default()).is_empty());
            assert_current_segment_visible(&mut c, &path, level);
        }
    }
}

fn assert_current_segment_visible(c: &mut Controller, path: &std::path::Path, level: ColorLevel) {
    let state = c.app.path_picker.as_ref().unwrap();
    let last = path_segments(path).len() - 1;
    assert_eq!((state.pane, state.selected), (1, last));
    assert!(state.scroll > 0);
    let theme = Theme::new(level);
    let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
    let mut g = ui::geometry(&c.app, Rect::new(0, 0, 80, 12));
    terminal
        .draw(|frame| g = ui::render(frame, &c.app, &theme))
        .unwrap();
    let layout = g.path_picker.as_ref().unwrap();
    let selected = layout
        .controls
        .iter()
        .filter(|(_, index)| *index == last)
        .collect::<Vec<_>>();
    assert!(!selected.is_empty());
    for (control, _) in selected {
        let cell = &terminal.backend().buffer()[(control.area.x, control.area.y)];
        assert_eq!(
            cell.fg,
            theme.style(Role::PathHover).fg.unwrap_or(Color::Reset)
        );
        assert_eq!(cell.modifier, theme.style(Role::PathHover).add_modifier);
    }
    assert!(
        c.handle(
            key(KeyCode::Enter, KeyModifiers::NONE),
            &g,
            Default::default()
        )
        .is_empty()
    );
    assert!(c.app.path_picker.is_none());
    assert_eq!(c.app.panes[1].path, path);
}

#[test]
fn alt_l_cannot_open_a_picker_over_an_operation_dialog() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path().into());
    c.app.complete_listing(
        0,
        1,
        Ok(terminal_file_manager::fs::list(
            root.path(),
            &terminal_file_manager::fs::IgnoreContext::isolated(),
            &false.into(),
        )
        .unwrap()),
    );
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    c.dispatch(Action::Mkdir, &g);
    assert!(c.app.operations.dialog.is_some());
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    assert!(
        c.handle(
            key(KeyCode::Char('l'), KeyModifiers::ALT),
            &g,
            Default::default()
        )
        .is_empty()
    );
    assert!(c.app.path_picker.is_none());
    assert!(c.app.operations.dialog.is_some());
    assert!(c.app.operations.outbox.is_empty());
}
