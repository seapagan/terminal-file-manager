//! Header navigation shares pane history and rejects stale/occluded hit targets.
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
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
