mod parent_identity;

use super::*;
use crate::{
    app::{PatternControl, PatternOperation, Request, RequestKind},
    browser::{submit_deferred_listings, tests::collect_until_idle},
    fs::IgnoreContext,
    input::{Action, Bindings},
    settings::Settings,
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend};
use std::{
    sync::{Arc, Mutex, atomic::Ordering, mpsc},
    time::Instant,
};

fn draw(terminal: &mut Terminal<TestBackend>, app: &App) -> ui::Geometry {
    let mut geometry = None;
    terminal
        .draw(|frame| {
            geometry = Some(ui::render(
                frame,
                app,
                &Theme::new(colored_text::ColorLevel::NoColor),
            ));
        })
        .unwrap();
    geometry.unwrap()
}

fn mark_enabled(geometry: &ui::Geometry) -> bool {
    let targets = geometry.pattern.as_ref().unwrap().targets();
    targets.contains(&PatternControl::Apply(PatternOperation::Mark))
}

#[test]
fn invalid_listing_completion_has_no_presentation_or_effect() {
    let mut app = App::new("owned".into());
    let before = listing_presentation(&app, 0);
    for pane in [2, usize::MAX] {
        assert_eq!(listing_presentation(&app, pane), None);
        app.complete_listing(pane, 1, Err("invalid pane".into()));
        assert_eq!(listing_presentation(&app, pane), None);
        assert_eq!(listing_presentation(&app, 0), before);
    }
}

#[test]
fn unchanged_refresh_reenables_rendered_pattern_apply_without_an_input_event() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = crate::browser::tests::counting_controller(root.path());
    let mut pool = Pool::new(crate::fs::IgnoreContext::isolated()).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let geometry = draw(&mut terminal, &controller.app);
    controller.dispatch(Action::PatternSelection, &geometry);
    controller.app.pattern.as_mut().unwrap().append("direct");
    let geometry = draw(&mut terminal, &controller.app);
    assert!(mark_enabled(&geometry));
    let installs = controller.app.panes[0].listing_installs;
    let request = controller.app.request_reconciliation(0).unwrap();
    controller.handle(Event::Paste("ory".into()), &geometry, Duration::ZERO);
    let geometry = draw(&mut terminal, &controller.app);
    assert!(!mark_enabled(&geometry));
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .contains("Listing unavailable or refreshing")
    );
    pool.submit(request);
    // This helper requires collect to request a redraw, as the runtime does.
    crate::browser::tests::collect_until_idle(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].listing_installs, installs);
    let geometry = draw(&mut terminal, &controller.app);
    assert!(mark_enabled(&geometry));
    controller.handle(
        Event::Key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT)),
        &geometry,
        Duration::ZERO,
    );
    assert!(controller.app.pattern.is_none());
    assert_eq!(controller.app.panes[0].selection_counts().visible, 1);
}

#[test]
fn unchanged_refresh_without_pattern_selection_does_not_request_redraw() {
    let root = tempfile::tempdir().unwrap();
    let mut controller = crate::browser::tests::counting_controller(root.path());
    let mut pool = Pool::new(crate::fs::IgnoreContext::isolated()).unwrap();
    unchanged_without_redraw(&mut controller, &mut pool);
}

fn creation_before_completion(root: &std::path::Path) -> (Controller, std::path::PathBuf) {
    let mut controller = crate::browser::tests::counting_controller(root);
    let created = root.join("created");
    controller.app.panes[0].track_creation(7, created.clone());
    std::fs::write(&created, "new").unwrap();
    let request = controller.app.request_reconciliation(0).unwrap();
    let snapshot = crate::fs::list(
        root,
        &crate::fs::IgnoreContext::isolated(),
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    controller
        .app
        .complete_listing(0, request.generation, Ok(snapshot));
    assert_ne!(controller.app.panes[0].cursor.as_ref(), Some(&created));
    (controller, created)
}

fn unchanged_without_redraw(controller: &mut Controller, pool: &mut Pool) {
    let installs = controller.app.panes[0].listing_installs;
    pool.submit(controller.app.request_reconciliation(0).unwrap());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while pool.active_count() != 0 {
        assert!(!collect(controller, pool));
        assert!(std::time::Instant::now() < deadline, "listing timed out");
        std::thread::yield_now();
    }
    assert_eq!(controller.app.panes[0].listing_installs, installs);
    assert!(!controller.app.panes[0].listing);
}

#[test]
fn unchanged_refresh_focuses_creation_and_redraws_before_an_unrelated_change() {
    let root = tempfile::tempdir().unwrap();
    let (mut controller, created) = creation_before_completion(root.path());
    controller.app.panes[0].finish_creation(7, true);
    let installs = controller.app.panes[0].listing_installs;
    let mut pool = Pool::new(crate::fs::IgnoreContext::isolated()).unwrap();
    pool.submit(controller.app.request_reconciliation(0).unwrap());
    crate::browser::tests::collect_until_idle(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].cursor.as_ref(), Some(&created));
    assert_eq!(controller.app.panes[0].listing_installs, installs);
    unchanged_without_redraw(&mut controller, &mut pool);

    controller.app.panes[0].move_cursor(-isize::MAX, 20);
    let cursor = controller.app.panes[0].cursor.clone();
    assert_ne!(cursor.as_ref(), Some(&created));
    std::fs::write(root.path().join("unrelated"), "change").unwrap();
    pool.submit(controller.app.request_reconciliation(0).unwrap());
    crate::browser::tests::collect_until_idle(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].cursor, cursor);
}

#[test]
fn unchanged_refresh_before_creation_completion_keeps_focus_pending() {
    let root = tempfile::tempdir().unwrap();
    let (mut controller, created) = creation_before_completion(root.path());
    let cursor = controller.app.panes[0].cursor.clone();
    let mut pool = Pool::new(crate::fs::IgnoreContext::isolated()).unwrap();
    unchanged_without_redraw(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].cursor, cursor);
    controller.app.panes[0].finish_creation(7, true);
    pool.submit(controller.app.request_reconciliation(0).unwrap());
    crate::browser::tests::collect_until_idle(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].cursor.as_ref(), Some(&created));
}

#[test]
fn user_navigation_overrides_creation_focus_even_after_returning_to_original_row() {
    for before_completion in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let (mut controller, created) = creation_before_completion(root.path());
        if !before_completion {
            controller.app.panes[0].finish_creation(7, true);
        }
        let cursor = controller.app.panes[0].cursor.clone();
        controller.app.panes[0].move_cursor(1, 20);
        controller.app.panes[0].move_cursor(-1, 20);
        assert_eq!(controller.app.panes[0].cursor, cursor);
        assert_ne!(cursor.as_ref(), Some(&created));
        if before_completion {
            controller.app.panes[0].finish_creation(7, true);
        }
        let mut pool = Pool::new(crate::fs::IgnoreContext::isolated()).unwrap();
        unchanged_without_redraw(&mut controller, &mut pool);
        std::fs::write(root.path().join("unrelated"), "change").unwrap();
        pool.submit(controller.app.request_reconciliation(0).unwrap());
        crate::browser::tests::collect_until_idle(&mut controller, &mut pool);
        assert_eq!(controller.app.panes[0].cursor, cursor);
    }
}

#[test]
fn hidden_sidebar_does_not_request_metadata_clock_redraw() {
    for (width, dual, popup, expected) in [
        (99, false, false, false),
        (100, false, false, true),
        (120, true, false, false),
        (80, false, true, true),
    ] {
        let mut app = App::new("owned".into());
        app.sidebar = true;
        app.dual = dual;
        app.popup = popup;
        app.metadata_formats[0] = Some(app.metadata_format(true));
        app.metadata_formats[0].as_mut().unwrap().fuzzy_timestamps = true;
        let geometry = ui::geometry(&app, ratatui::layout::Rect::new(0, 0, width, 24));
        assert_eq!(geometry.sidebar.is_some(), !dual && width >= 100);
        app.now = std::time::SystemTime::UNIX_EPOCH;
        assert_eq!(
            tick(&mut app, width),
            expected,
            "width={width}, dual={dual}, popup={popup}"
        );
        app.panes[0].fuzzy_timestamps = true;
        app.now = std::time::SystemTime::UNIX_EPOCH;
        assert!(
            tick(&mut app, width),
            "listing timestamps still need updates"
        );
    }
}

#[test]
fn parent_inputs_before_startup_listing_return_focus_to_the_exited_child() {
    use ParentInput::*;
    for input in [
        Key(KeyCode::Enter, KeyModifiers::NONE),
        Key(KeyCode::Backspace, KeyModifiers::NONE),
        Key(KeyCode::Up, KeyModifiers::ALT),
        Breadcrumb,
        Picker,
        DoubleClick,
    ] {
        assert_startup_parent_focus(input);
    }
}

#[derive(Debug)]
enum ParentInput {
    Key(KeyCode, KeyModifiers),
    Breadcrumb,
    Picker,
    DoubleClick,
}

fn assert_startup_parent_focus(input: ParentInput) {
    let (root, mut controller) = startup_child();
    controller.app.dual = true;
    let child = root.path().join("child");
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 240, 24));
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let startup = Request {
        pane: 0,
        generation: 1,
        path: child.clone(),
        kind: RequestKind::Listing,
    };
    let (release, cancelled) = block_listing(&mut pool, startup);
    assert!(controller.app.panes[0].parent_focused());
    let requests = parent_input(&mut controller, &geometry, &input, root.path());
    assert_eq!(
        requests.len(),
        1,
        "{input:?} must submit Parent immediately"
    );
    pool.submit(requests.into_iter().next().unwrap());
    controller.dispatch(Action::FocusPane, &geometry);
    assert_eq!(controller.app.active, 1);
    collect_until(&mut controller, &mut pool, |app| {
        app.panes[0].path == root.path()
    });
    assert_eq!(controller.app.panes[0].path, root.path());
    release.send(()).unwrap();
    assert!(cancelled.recv_timeout(Duration::from_secs(10)).unwrap());
    drain_stale_listing(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].cursor, Some(child));
    assert!(controller.app.panes[0].error.is_none());
}

fn parent_input(
    controller: &mut Controller,
    geometry: &ui::Geometry,
    input: &ParentInput,
    parent: &std::path::Path,
) -> Vec<Request> {
    let event = match *input {
        ParentInput::Key(code, modifiers) => Event::Key(KeyEvent::new(code, modifiers)),
        ParentInput::Breadcrumb => breadcrumb(geometry, parent),
        ParentInput::Picker => return picker_enter(controller, geometry, true),
        ParentInput::DoubleClick => {
            let pane = geometry.panes[0].unwrap();
            let click = mouse_click(pane.x + 8, pane.y + 2);
            assert!(
                controller
                    .handle(click.clone(), geometry, Duration::ZERO)
                    .is_empty()
            );
            click
        }
    };
    controller.handle(event, geometry, Duration::ZERO)
}

#[test]
fn parent_after_startup_listing_failure_restores_the_child_by_identity() {
    let (root, mut controller) = startup_child();
    let child = root.path().join("child");
    let app = &mut controller.app;
    assert!(app.complete_listing(0, 1, Err("owned startup failure".into())));
    assert!(app.panes[0].error.is_some());
    let parent = app.parent().unwrap();
    assert_eq!(parent.path, root.path());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    pool.submit(parent);
    collect_until_idle(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].path, root.path());
    assert_eq!(controller.app.panes[0].cursor, Some(child));
    assert!(controller.app.panes[0].error.is_none());
}

#[test]
fn current_directory_selection_supersedes_parent_navigation() {
    for picker in [false, true] {
        let (_root, mut controller) = startup_child();
        let child = controller.app.panes[0].path.clone();
        let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
        let parent = controller.app.parent().unwrap();
        let (release, cancelled) = block_listing(&mut pool, parent);
        let area = ratatui::layout::Rect::new(0, 0, 160, 24);
        let geometry = ui::geometry(&controller.app, area);
        let current = if picker {
            picker_enter(&mut controller, &geometry, false)
        } else {
            controller.handle(breadcrumb(&geometry, &child), &geometry, Duration::ZERO)
        }
        .remove(0);
        assert_eq!(current.path, child);
        pool.submit(current);
        collect_until(&mut controller, &mut pool, |app| !app.panes[0].listing);
        release.send(()).unwrap();
        assert!(cancelled.recv_timeout(Duration::from_secs(10)).unwrap());
        drain_stale_listing(&mut controller, &mut pool);
        assert_eq!(controller.app.panes[0].path, child);
        assert!(controller.app.panes[0].parent_focused());
        assert!(controller.app.panes[0].error.is_none());
        assert!(controller.app.history(false).is_none());
    }
}

#[test]
fn ordinary_navigation_completes_while_a_superseded_listing_is_blocked() {
    let (root, mut controller) = startup_child();
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let child = controller.app.panes[0].path.clone();
    let initial = controller.app.request_listing(0, child);
    let (release, cancelled) = block_listing(&mut pool, initial);
    pool.submit(controller.app.request_listing(0, root.path().into()));
    assert_eq!(
        pool.active_count(),
        2,
        "replacement must start before release"
    );
    collect_until(&mut controller, &mut pool, |app| {
        app.panes[0].path == root.path()
    });
    assert_eq!(pool.active_count(), 1);
    release.send(()).unwrap();
    assert!(cancelled.recv_timeout(Duration::from_secs(10)).unwrap());
    drain_stale_listing(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].path, root.path());
    assert!(controller.app.panes[0].error.is_none());
}

fn picker_enter(
    controller: &mut Controller,
    geometry: &ui::Geometry,
    parent: bool,
) -> Vec<Request> {
    controller.dispatch(Action::FullPath, geometry);
    if parent {
        let picker = ui::geometry(&controller.app, geometry.area);
        let left = Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        controller.handle(left, &picker, Duration::ZERO);
    }
    let picker = ui::geometry(&controller.app, geometry.area);
    let enter = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    controller.handle(enter, &picker, Duration::ZERO)
}

fn breadcrumb(geometry: &ui::Geometry, path: &std::path::Path) -> Event {
    let target = geometry
        .path_controls
        .iter()
        .find(|control| control.pane == 0 && control.target.as_deref() == Some(path))
        .unwrap();
    mouse_click(target.area.x, target.area.y)
}

fn mouse_click(column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn parent_cancels_earlier_refresh_but_later_refresh_follows_it() {
    for refresh_after_parent in [false, true] {
        let (root, mut controller) = startup_child();
        if !refresh_after_parent {
            assert!(controller.app.request_refresh(0).is_none());
            assert!(controller.app.panes[0].refresh_pending);
        }
        let parent = controller.app.parent().unwrap();
        if refresh_after_parent {
            assert!(controller.app.request_refresh(0).is_none());
            assert!(controller.app.status.starts_with("Refresh queued"));
        } else {
            assert_eq!(
                &*controller.app.status,
                "Deferred refresh cancelled by newer navigation"
            );
        }
        let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
        pool.submit(parent);
        collect_until_idle(&mut controller, &mut pool);
        assert_eq!(controller.app.panes[0].path, root.path());
        submit_deferred_listings(&mut controller.app, &mut pool);
        assert_eq!(pool.active_count(), usize::from(refresh_after_parent));
        if refresh_after_parent {
            collect_until(&mut controller, &mut pool, |app| !app.panes[0].listing);
            assert_eq!(
                controller.app.panes[0].cursor,
                Some(root.path().join("child"))
            );
            assert!(controller.app.panes[0].error.is_none());
        }
    }
}

fn startup_child() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let controller = Controller::new(
        App::new(child),
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    (root, controller)
}

fn block_listing(pool: &mut Pool, request: Request) -> (mpsc::Sender<()>, mpsc::Receiver<bool>) {
    let (started, release, cancelled) =
        hold_request(pool, request.generation, request.kind.clone());
    pool.submit(request);
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    (release, cancelled)
}

fn hold_request(
    pool: &mut Pool,
    generation: u64,
    kind: RequestKind,
) -> (mpsc::Receiver<()>, mpsc::Sender<()>, mpsc::Receiver<bool>) {
    let (entered, started) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let (stopped, cancelled_on_release) = mpsc::channel();
    let blocked = Mutex::new(blocked);
    pool.before_execute = Some(Arc::new(move |request, cancelled| {
        if request.generation == generation && request.kind == kind {
            entered.send(()).unwrap();
            if blocked.lock().unwrap().recv().is_ok() {
                stopped.send(cancelled.load(Ordering::Relaxed)).unwrap();
            }
        }
    }));
    (started, release, cancelled_on_release)
}

fn collect_until(controller: &mut Controller, pool: &mut Pool, ready: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready(&controller.app) {
        assert!(
            Instant::now() < deadline,
            "navigation did not complete while another worker was blocked"
        );
        collect(controller, pool);
        std::thread::yield_now();
    }
}

fn drain_stale_listing(controller: &mut Controller, pool: &mut Pool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while pool.active_count() != 0 {
        assert!(Instant::now() < deadline, "cancelled listing did not stop");
        collect(controller, pool);
        std::thread::yield_now();
    }
}
