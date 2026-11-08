//! Cursor input must retain the viewport actually drawn after parent restoration.
use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Modifier};
use std::{path::Path, time::Duration};
use terminal_file_manager::{
    app::{App, Request},
    color::Theme,
    fs::{self, IgnoreContext},
    input::{Action, Bindings, Controller},
    ui,
};

const WIDTH: u16 = 240;
const HEIGHT: u16 = 12;

fn fixture() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    for index in 0..40 {
        std::fs::create_dir(root.path().join(format!("child{index:02}"))).unwrap();
    }
    let child = root.path().join("child30");
    std::fs::write(child.join("inside"), "owned").unwrap();
    let mut app = App::new(child.clone());
    app.dual = true;
    for pane in 0..2 {
        let snapshot = fs::list(&child, &IgnoreContext::isolated(), &false.into()).unwrap();
        assert!(app.complete_listing(pane, 1, Ok(snapshot)));
    }
    app.panes[1].select_all();
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        ),
    )
}

fn complete(c: &mut Controller, request: Request) {
    let snapshot = fs::list(&request.path, &IgnoreContext::isolated(), &false.into()).unwrap();
    assert!(
        c.app
            .complete_listing(request.pane, request.generation, Ok(snapshot))
    );
}

fn draw(c: &Controller, height: u16) -> (Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, height)).unwrap();
    let mut geometry = ui::geometry(&c.app, terminal.backend().buffer().area);
    terminal
        .draw(|frame| geometry = ui::render(frame, &c.app, &Theme::new(ColorLevel::NoColor)))
        .unwrap();
    (terminal.backend().buffer().clone(), geometry)
}

fn key(
    c: &mut Controller,
    geometry: &ui::Geometry,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Vec<Request> {
    c.handle(
        Event::Key(KeyEvent::new(code, modifiers)),
        geometry,
        Duration::ZERO,
    )
}

fn parent(c: &mut Controller, root: &Path, route: &str) {
    let (_, geometry) = draw(c, HEIGHT);
    let requests = match route {
        "breadcrumb" => {
            let control = geometry
                .path_controls
                .iter()
                .find(|p| p.pane == 0 && p.target.as_deref() == Some(root))
                .unwrap();
            c.handle(
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: control.area.x,
                    row: control.area.y,
                    modifiers: KeyModifiers::NONE,
                }),
                &geometry,
                Duration::ZERO,
            )
        }
        "backspace" => key(c, &geometry, KeyCode::Backspace, KeyModifiers::NONE),
        _ => key(c, &geometry, KeyCode::Up, KeyModifiers::ALT),
    };
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, root);
    complete(c, requests.into_iter().next().unwrap());
    assert_eq!(
        c.app.panes[0].cursor.as_deref(),
        Some(root.join("child30").as_path())
    );
}

fn names(buffer: &Buffer, geometry: &ui::Geometry) -> Vec<String> {
    let pane = geometry.panes[0].unwrap();
    (0..geometry.listing_rows[0])
        .map(|row| {
            let text = (pane.x + 8..pane.right() - 1)
                .map(|x| buffer[(x, pane.y + 2 + row as u16)].symbol())
                .collect::<String>();
            text.split_whitespace().next().unwrap_or("").to_owned()
        })
        .collect()
}

fn click(c: &mut Controller, geometry: &ui::Geometry, offset: usize) {
    let pane = geometry.panes[0].unwrap();
    assert!(
        c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: pane.x + 8,
                row: pane.y + 2 + offset as u16,
                modifiers: KeyModifiers::NONE
            }),
            geometry,
            Duration::from_secs(1)
        )
        .is_empty()
    );
}

fn cursor_row(buffer: &Buffer, geometry: &ui::Geometry, offset: usize) {
    let pane = geometry.panes[0].unwrap();
    assert!(
        buffer[(pane.x + 8, pane.y + 2 + offset as u16)]
            .modifier
            .contains(Modifier::REVERSED)
    );
}

#[test]
fn initial_parent_keyboard_movement_keeps_the_drawn_viewport_in_each_layout() {
    for route in ["breadcrumb", "backspace", "alt-up"] {
        for layout in ["single", "dual", "maximised"] {
            let (root, mut c) = fixture();
            c.app.dual = layout != "single";
            c.app.maximized = layout == "maximised";
            parent(&mut c, root.path(), route);
            let (before, geometry) = draw(&c, HEIGHT);
            let height = geometry.listing_rows[0];
            assert!(height >= 3);
            cursor_row(&before, &geometry, height - 1);
            let shown = names(&before, &geometry);
            assert_eq!(shown.last().unwrap(), "child30/");
            key(&mut c, &geometry, KeyCode::Up, KeyModifiers::NONE);
            let (after, next) = draw(&c, HEIGHT);
            assert_eq!(names(&after, &next), shown, "{route}/{layout}");
            assert_eq!(
                c.app.panes[0].cursor.as_deref(),
                Some(root.path().join("child29").as_path())
            );
            cursor_row(&after, &next, height - 2);
            key(&mut c, &next, KeyCode::Down, KeyModifiers::NONE);
            let (after, next) = draw(&c, HEIGHT);
            assert_eq!(names(&after, &next), shown);
            cursor_row(&after, &next, height - 1);
            assert_eq!(c.app.panes[1].marks.len(), 1);
        }
    }
}

#[test]
fn initial_parent_clicks_keep_target_rows_fixed_for_following_cursor_input() {
    for route in ["breadcrumb", "backspace", "alt-up"] {
        for offset in [0, 1, 2] {
            let (root, mut c) = fixture();
            parent(&mut c, root.path(), route);
            let (before, geometry) = draw(&c, HEIGHT);
            let shown = names(&before, &geometry);
            let start = c.app.panes[0].viewport_start(geometry.listing_rows[0]);
            let expected = c.app.panes[0].row_path(start + offset).unwrap();
            click(&mut c, &geometry, offset);
            assert_eq!(c.app.panes[0].cursor.as_ref(), Some(&expected));
            let (after, next) = draw(&c, HEIGHT);
            assert_eq!(names(&after, &next), shown, "{route}/{offset}");
            cursor_row(&after, &next, offset);
            key(&mut c, &next, KeyCode::Down, KeyModifiers::NONE);
            let (after, next) = draw(&c, HEIGHT);
            assert_eq!(names(&after, &next), shown);
            cursor_row(&after, &next, offset + 1);
        }
    }
}

#[test]
fn keyboard_scrolls_one_row_only_when_the_cursor_crosses_a_drawn_boundary() {
    let (root, mut c) = fixture();
    parent(&mut c, root.path(), "backspace");
    let (before, geometry) = draw(&c, HEIGHT);
    let height = geometry.listing_rows[0];
    let shown = names(&before, &geometry);
    key(&mut c, &geometry, KeyCode::Down, KeyModifiers::NONE);
    let (after, geometry) = draw(&c, HEIGHT);
    let shifted = names(&after, &geometry);
    assert_eq!(shifted[..height - 1], shown[1..]);
    cursor_row(&after, &geometry, height - 1);
    for offset in (0..height - 1).rev() {
        key(&mut c, &geometry, KeyCode::Up, KeyModifiers::NONE);
        let (after, next) = draw(&c, HEIGHT);
        assert_eq!(names(&after, &next), shifted);
        cursor_row(&after, &next, offset);
    }
    key(&mut c, &geometry, KeyCode::Up, KeyModifiers::NONE);
    let (after, next) = draw(&c, HEIGHT);
    let shifted_back = names(&after, &next);
    assert_eq!(shifted_back[1..], shifted[..height - 1]);
    cursor_row(&after, &next, 0);
}

#[test]
fn range_selection_from_restored_parent_uses_the_drawn_viewport() {
    let (root, mut c) = fixture();
    parent(&mut c, root.path(), "alt-up");
    let (before, geometry) = draw(&c, HEIGHT);
    let height = geometry.listing_rows[0];
    let shown = names(&before, &geometry);
    let target = c.app.panes[0]
        .row_path(c.app.panes[0].viewport_start(height))
        .unwrap();
    c.app.panes[0].select_range_to(target.clone(), height);
    let (after, next) = draw(&c, HEIGHT);
    assert_eq!(names(&after, &next), shown);
    assert_eq!(c.app.panes[0].cursor.as_ref(), Some(&target));
    assert_eq!(c.app.panes[0].marks.len(), height);
    cursor_row(&after, &next, 0);
    c.app.panes[0].extend_range(1, height);
    let (after, next) = draw(&c, HEIGHT);
    assert_eq!(names(&after, &next), shown);
    cursor_row(&after, &next, 1);
}

#[test]
fn parent_refresh_keeps_marks_and_the_displayed_viewport_stable_for_movement() {
    let (root, mut c) = fixture();
    parent(&mut c, root.path(), "breadcrumb");
    c.app.panes[0].toggle_mark();
    let marks = c.app.panes[0].marks.clone();
    let (before, geometry) = draw(&c, HEIGHT);
    let shown = names(&before, &geometry);
    let refresh = c.app.request_refresh(0).unwrap();
    complete(&mut c, refresh);
    let (after, geometry) = draw(&c, HEIGHT);
    assert_eq!(names(&after, &geometry), shown);
    assert_eq!(c.app.panes[0].marks, marks);
    key(&mut c, &geometry, KeyCode::Up, KeyModifiers::NONE);
    let (after, next) = draw(&c, HEIGHT);
    assert_eq!(names(&after, &next), shown);
    assert_eq!(c.app.panes[0].marks, marks);
    cursor_row(&after, &next, next.listing_rows[0] - 2);
    assert_eq!(geometry.listing_rows[0], next.listing_rows[0]);
}

#[test]
fn history_round_trip_retains_the_viewport_and_cursor_selected_after_parent() {
    let (root, mut c) = fixture();
    parent(&mut c, root.path(), "backspace");
    let (_, geometry) = draw(&c, HEIGHT);
    click(&mut c, &geometry, 2);
    let (before, geometry) = draw(&c, HEIGHT);
    let shown = names(&before, &geometry);
    let selected = c.app.panes[0].cursor.clone();
    let request = c.dispatch(Action::Enter, &geometry).remove(0);
    complete(&mut c, request);
    let request = c.app.history(false).unwrap();
    complete(&mut c, request);
    let (after, geometry) = draw(&c, HEIGHT);
    assert_eq!(c.app.panes[0].cursor, selected);
    assert_eq!(names(&after, &geometry), shown);
    cursor_row(&after, &geometry, 2);
    key(&mut c, &geometry, KeyCode::Up, KeyModifiers::NONE);
    let (after, geometry) = draw(&c, HEIGHT);
    assert_eq!(names(&after, &geometry), shown);
    cursor_row(&after, &geometry, 1);
    let cursor = c.app.panes[0].cursor.clone();
    let request = c.app.history(true).unwrap();
    complete(&mut c, request);
    assert_eq!(c.app.panes[0].path.as_path(), selected.as_deref().unwrap());
    let request = c.app.history(false).unwrap();
    complete(&mut c, request);
    let (after, geometry) = draw(&c, HEIGHT);
    assert_eq!(c.app.panes[0].cursor, cursor);
    assert_eq!(names(&after, &geometry), shown);
    cursor_row(&after, &geometry, 1);
}
