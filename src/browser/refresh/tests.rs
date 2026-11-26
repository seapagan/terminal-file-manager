use super::*;
use crate::{
    app::{PatternControl, PatternOperation, Request, RequestKind},
    browser::{submit_deferred_listings, tests::collect_until_idle},
    fs::IgnoreContext,
    input::{Action, Bindings},
    settings::Settings,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

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
fn parent_keys_before_startup_listing_return_focus_to_the_exited_child() {
    for key in [
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
    ] {
        assert_startup_parent_focus(key);
    }
}

fn assert_startup_parent_focus(key: KeyEvent) {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let mut controller = Controller::new(
        App::new(child.clone()),
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 100, 24));
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    pool.submit(Request {
        pane: 0,
        generation: 1,
        path: child.clone(),
        kind: RequestKind::Listing,
    });
    assert!(controller.app.panes[0].parent_focused());
    // Process the key while the initial listing is still awaiting collection.
    assert!(
        controller
            .handle(Event::Key(key), &geometry, Duration::ZERO)
            .is_empty(),
        "{key:?} superseded the initial listing"
    );
    controller.app.active = 1;
    collect_until_idle(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].path, child);
    submit_deferred_listings(&mut controller.app, &mut pool);
    collect_until_idle(&mut controller, &mut pool);
    assert_eq!(controller.app.panes[0].path, root.path());
    assert_eq!(controller.app.panes[0].cursor, Some(child));
}

#[test]
fn refresh_during_deferred_startup_navigation_follows_the_parent() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let listing = |path: &std::path::Path| {
        crate::fs::list(
            path,
            &IgnoreContext::isolated(),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .map_err(|error| error.to_string())
    };
    let mut app = App::new(child.clone());
    assert!(app.parent().is_none());
    assert!(app.request_refresh(0).is_none());
    assert!(app.complete_listing(0, 1, listing(&child)));
    assert!(app.take_refresh(0).is_none());
    let parent = app.take_startup_parent(0).unwrap();
    assert_eq!(parent.path, root.path());
    assert!(app.complete_listing(0, parent.generation, listing(&parent.path)));
    assert_eq!(app.take_refresh(0).unwrap().path, root.path());
}
