use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Action, Bindings, Controller},
    settings::Settings,
    ui,
};

fn controller(path: &std::path::Path) -> Controller {
    let mut app = App::new(path.into());
    let snapshot = terminal_file_manager::fs::list(
        path,
        &terminal_file_manager::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    app.complete_listing(0, 1, Ok(snapshot));
    Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    )
}

#[test]
fn parent_row_is_keyboard_accessible_and_never_a_filesystem_operand() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), "owned").unwrap();
    let mut c = controller(root.path());
    let g = ui::geometry(&c.app, Rect::new(0, 0, 100, 24));
    c.dispatch(Action::Home, &g);
    assert!(
        c.app.panes[0].current().is_none(),
        "Home must focus virtual parent"
    );
    c.dispatch(Action::Mark, &g);
    assert!(c.app.panes[0].marks.is_empty());
    assert!(c.app.panes[0].eligible().is_empty());
    assert_eq!(c.app.panes[0].entries.len(), 1);
    let requests = c.handle(
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &g,
        Default::default(),
    );
    assert_eq!(requests[0].path, root.path().parent().unwrap());
}

#[test]
fn parent_row_remains_rendered_when_filter_has_no_matches() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    c.app.panes[0].set_filter("absent".into());
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, &c.app, &Theme::new(colored_text::ColorLevel::NoColor));
        })
        .unwrap();
    let row = (0..80)
        .map(|x| terminal.backend().buffer()[(x, 2)].symbol())
        .collect::<String>();
    assert!(row.contains(".."), "{row}");
    assert!(c.app.panes[0].visible.is_empty());
}

fn mouse(
    c: &mut Controller,
    x: u16,
    y: u16,
    kind: crossterm::event::MouseEventKind,
) -> Vec<terminal_file_manager::app::Request> {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
    c.handle(
        Event::Mouse(crossterm::event::MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }),
        &g,
        Default::default(),
    )
}

#[test]
fn clicking_a_header_ancestor_requests_its_native_path() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("child");
    std::fs::create_dir(&path).unwrap();
    let mut c = controller(&path);
    let requests = mouse(
        &mut c,
        11,
        0,
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
    );
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, path.ancestors().last().unwrap());
}

#[test]
fn ellipsis_opens_a_keyboard_picker_and_escape_captures_input() {
    let path = std::path::PathBuf::from(format!("/alpha/{}/leaf", "long".repeat(40)));
    let mut c = Controller::new(
        App::new(path.clone()),
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, &c.app, &Theme::new(colored_text::ColorLevel::NoColor));
        })
        .unwrap();
    let x = (0..80)
        .find(|x| terminal.backend().buffer()[(*x, 0)].symbol() == "…")
        .unwrap();
    mouse(
        &mut c,
        x,
        0,
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
    );
    assert!(c.app.path_picker.is_some());
    let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
    assert!(
        c.handle(
            Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            &g,
            Default::default()
        )
        .is_empty()
    );
    assert!(!c.quit);
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        &g,
        Default::default(),
    );
    assert!(c.app.path_picker.is_none());
    assert_eq!(c.app.panes[0].path, path);
}

#[test]
fn parent_single_click_focuses_and_double_click_uses_existing_parent_restoration() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let mut c = controller(&child);
    let original = c.app.panes[1].path.clone();
    let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
    for (time, navigates) in [(0, false), (100, true)] {
        let requests = c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 8,
                row: 2,
                modifiers: KeyModifiers::NONE,
            }),
            &g,
            std::time::Duration::from_millis(time),
        );
        assert_eq!(!requests.is_empty(), navigates);
        assert!(c.app.panes[0].current().is_none());
        if let Some(request) = requests.first() {
            assert_eq!(request.path, root.path());
            let snapshot = terminal_file_manager::fs::list(
                &request.path,
                &terminal_file_manager::fs::IgnoreContext::isolated(),
                &false.into(),
            )
            .unwrap();
            c.app.complete_listing(0, request.generation, Ok(snapshot));
            assert_eq!(c.app.panes[0].cursor.as_deref(), Some(child.as_path()));
        }
    }
    assert_eq!(c.app.panes[1].path, original);
}

#[test]
fn root_has_no_virtual_parent_and_parent_focus_survives_refresh_and_history() {
    let root = tempfile::tempdir().unwrap();
    let filesystem_root = root.path().ancestors().last().unwrap();
    let app = App::new(filesystem_root.into());
    assert!(!app.panes[0].has_parent());
    assert_eq!(app.panes[0].row_count(), 0);
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let mut c = controller(&child);
    let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
    c.dispatch(Action::Home, &g);
    let request = c.app.request_refresh(0).unwrap();
    let snapshot = terminal_file_manager::fs::list(
        &child,
        &terminal_file_manager::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    c.app.complete_listing(0, request.generation, Ok(snapshot));
    assert!(c.app.panes[0].parent_focused());
    let request = c.dispatch(Action::Enter, &g).remove(0);
    let snapshot = terminal_file_manager::fs::list(
        &request.path,
        &terminal_file_manager::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    c.app.complete_listing(0, request.generation, Ok(snapshot));
    let request = c.dispatch(Action::Back, &g).remove(0);
    let snapshot = terminal_file_manager::fs::list(
        &request.path,
        &terminal_file_manager::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    c.app.complete_listing(0, request.generation, Ok(snapshot));
    assert!(c.app.panes[0].parent_focused());
}

#[test]
fn operation_shortcuts_on_parent_cannot_submit_a_filesystem_mutation() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("owned"), "unchanged").unwrap();
    for action in [
        Action::Copy,
        Action::Move,
        Action::Trash,
        Action::Delete,
        Action::Rename,
        Action::Attributes,
        Action::Count,
    ] {
        let mut c = controller(root.path());
        let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
        c.dispatch(Action::Home, &g);
        assert!(c.dispatch(action, &g).is_empty());
        assert!(c.app.operations.outbox.is_empty());
        assert!(c.app.operations.dialog.is_none());
        assert!(c.app.panes[0].marks.is_empty());
        assert_eq!(
            std::fs::read(root.path().join("owned")).unwrap(),
            b"unchanged"
        );
    }
}

#[test]
fn shift_clicking_parent_focuses_without_marking_or_double_click_navigation() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
    for time in [0, 100] {
        let event = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 8,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        });
        assert!(
            c.handle(event, &g, std::time::Duration::from_millis(time))
                .is_empty()
        );
        assert!(c.app.panes[0].parent_focused());
        assert!(c.app.panes[0].marks.is_empty());
        assert!(c.app.panes[0].pending.is_none());
    }
}
