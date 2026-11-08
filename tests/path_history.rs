//! Header navigation shares pane history and rejects stale/occluded hit targets.
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use terminal_file_manager::{
    app::{App, Request},
    fs::{self, IgnoreContext},
    input::{Bindings, Controller},
    settings::Settings,
    ui,
};

fn complete(c: &mut Controller, request: Request) {
    let snapshot = fs::list(&request.path, &IgnoreContext::isolated(), &false.into()).unwrap();
    assert!(
        c.app
            .complete_listing(request.pane, request.generation, Ok(snapshot))
    );
}

fn event(control: &ui::PathControl) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: control.area.x,
        row: control.area.y,
        modifiers: KeyModifiers::NONE,
    })
}

fn fixture() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("child").join("leaf");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("cursor"), "owned").unwrap();
    let mut app = App::new(path.clone());
    app.dual = true;
    let snapshot = fs::list(&path, &IgnoreContext::isolated(), &false.into()).unwrap();
    for pane in 0..2 {
        app.complete_listing(pane, 1, Ok(snapshot.clone()));
    }
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Settings::default(),
        ),
    )
}

#[test]
fn ancestor_clicks_keep_independent_focus_marks_and_back_forward_cursor_history() {
    let (root, mut c) = fixture();
    let initial = c.app.panes[1].path.clone();
    c.app.panes[0].select_all();
    let cursor = c.app.panes[1].cursor.clone();
    let g = ui::geometry(&c.app, Rect::new(0, 0, 240, 24));
    let control = g
        .path_controls
        .iter()
        .find(|p| p.pane == 1 && p.target.as_deref() == Some(root.path()))
        .unwrap();
    let request = c.handle(event(control), &g, Default::default()).remove(0);
    assert_eq!(c.app.active, 1);
    complete(&mut c, request);
    assert_eq!(c.app.panes[1].path, root.path());
    assert_eq!(c.app.panes[0].path, initial);
    assert_eq!(c.app.panes[0].marks.len(), 1);
    let back = c.app.history(false).unwrap();
    complete(&mut c, back);
    assert_eq!(c.app.panes[1].path, initial);
    assert_eq!(c.app.panes[1].cursor, cursor);
    let forward = c.app.history(true).unwrap();
    complete(&mut c, forward);
    assert_eq!(c.app.panes[1].path, root.path());
}

#[test]
fn modal_and_divider_input_never_activate_background_path_targets() {
    let (_root, mut c) = fixture();
    let g = ui::geometry(&c.app, Rect::new(0, 0, 240, 24));
    let control = g.path_controls[0].clone();
    let before = c.app.panes[0].cursor.clone();
    c.app.popup = true;
    let popup = ui::geometry(&c.app, Rect::new(0, 0, 240, 24));
    assert!(
        c.handle(event(&control), &popup, Default::default())
            .is_empty()
    );
    assert_eq!(c.app.panes[0].cursor, before);
    assert!(c.app.panes[0].pending.is_none());
    c.app.popup = false;
    let divider = g.divider.unwrap();
    let divider_event = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: divider.x,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert!(c.handle(divider_event, &g, Default::default()).is_empty());
    assert!(c.app.panes[0].pending.is_none());
}

#[test]
fn stale_header_geometry_cannot_navigate_from_a_replaced_source_path() {
    let (_root, mut c) = fixture();
    let g = ui::geometry(&c.app, Rect::new(0, 0, 240, 24));
    let control = g.path_controls[0].clone();
    c.app.panes[0].path.push("new");
    assert!(c.handle(event(&control), &g, Default::default()).is_empty());
    assert!(c.app.panes[0].pending.is_none());
}

fn key(c: &mut Controller, code: KeyCode, modifiers: KeyModifiers) -> Vec<Request> {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 240, 24));
    c.handle(
        Event::Key(KeyEvent::new(code, modifiers)),
        &g,
        Default::default(),
    )
}

fn open_picker(c: &mut Controller, help: bool) {
    assert!(key(c, KeyCode::Char('l'), KeyModifiers::ALT).is_empty());
    if help {
        assert!(key(c, KeyCode::F(1), KeyModifiers::NONE).is_empty());
    }
    assert!(c.app.path_picker.is_some());
}

fn reject(c: &mut Controller, request: &Request) {
    let snapshot = fs::list(&request.path, &IgnoreContext::isolated(), &false.into()).unwrap();
    assert!(
        !c.app
            .complete_listing(request.pane, request.generation, Ok(snapshot))
    );
}

fn parent_click(g: &ui::Geometry, pane: usize) -> Event {
    let area = g.panes[pane].unwrap();
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x + 8,
        row: area.y + 2,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn bound_navigation_completion_closes_picker_before_the_next_keyboard_or_mouse_event() {
    for help in [false, true] {
        for keyboard in [false, true] {
            let (root, mut c) = fixture();
            c.app.active = 1;
            let initial = c.app.panes[0].path.clone();
            let stale = c.app.request_listing(1, root.path().join("child"));
            let request = c.app.request_listing(1, root.path().into());
            open_picker(&mut c, help);
            c.app.path_pointer = Some((20, 10));
            reject(&mut c, &stale);
            assert!(c.app.path_picker.is_some());
            complete(&mut c, request);
            assert!(c.app.path_picker.is_none());
            assert!(c.app.context_help.is_none());
            assert!(c.app.path_pointer.is_none());
            let g = ui::geometry(&c.app, Rect::new(0, 0, 240, 24));
            assert!(g.path_picker.is_none());
            let input = if keyboard {
                Event::Key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE))
            } else {
                parent_click(&g, 1)
            };
            assert!(c.handle(input, &g, Default::default()).is_empty());
            assert!(c.app.panes[1].parent_focused());
            assert_eq!(c.app.panes[0].path, initial);
        }
    }
}

#[test]
fn failed_navigation_refresh_and_other_pane_completion_keep_the_picker_and_help() {
    for (pane, fail, refresh) in [(0, true, false), (0, false, true), (1, false, false)] {
        let (root, mut c) = fixture();
        open_picker(&mut c, true);
        let initial = c.app.path_picker.as_ref().unwrap().clone();
        let request = if refresh {
            c.app.request_refresh(pane).unwrap()
        } else {
            c.app.request_listing(pane, root.path().into())
        };
        if fail {
            assert!(
                c.app
                    .complete_listing(pane, request.generation, Err("owned failure".into()))
            );
        } else {
            complete(&mut c, request);
        }
        let picker = c.app.path_picker.as_ref().unwrap();
        assert_eq!(picker.path, initial.path);
        assert_eq!(picker.selected, initial.selected);
        assert_eq!(picker.scroll, initial.scroll);
        assert!(c.app.context_help.is_some());
    }
}

fn choose_current(c: &mut Controller, route: usize) -> Vec<Request> {
    if route != 0 {
        open_picker(c, false);
    }
    if route == 1 {
        return key(c, KeyCode::Enter, KeyModifiers::NONE);
    }
    let g = ui::geometry(&c.app, Rect::new(0, 0, 240, 24));
    let controls = if route == 0 {
        g.path_controls.iter().collect::<Vec<_>>()
    } else {
        g.path_picker
            .as_ref()
            .unwrap()
            .controls
            .iter()
            .map(|(control, _)| control)
            .collect()
    };
    let control = controls
        .into_iter()
        .find(|control| control.pane == 0 && control.target.as_ref() == Some(&c.app.panes[0].path))
        .unwrap();
    c.handle(event(control), &g, Default::default())
}

#[test]
fn current_directory_choice_supersedes_pending_header_and_keyboard_or_mouse_picker_navigation() {
    for route in 0..3 {
        let (root, mut c) = fixture();
        let initial = c.app.panes[0].path.clone();
        c.app.panes[0].select_all();
        let marks = c.app.panes[0].marks.clone();
        let stale = c.app.request_listing(0, root.path().into());
        let requests = choose_current(&mut c, route);
        assert_eq!(requests.len(), 1);
        let request = requests.into_iter().next().unwrap();
        assert_eq!(request.path, initial);
        assert!(request.generation > stale.generation);
        reject(&mut c, &stale);
        assert_eq!(c.app.panes[0].pending.as_ref(), Some(&initial));
        complete(&mut c, request);
        reject(&mut c, &stale);
        assert_eq!(c.app.panes[0].path, initial);
        assert_eq!(c.app.panes[0].marks, marks);
        assert!(c.app.panes[0].pending.is_none());
        assert!(c.app.path_picker.is_none());
        assert!(c.app.history(false).is_none());
    }
}
