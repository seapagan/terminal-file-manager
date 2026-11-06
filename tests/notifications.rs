use terminal_file_manager::settings::Settings;

#[test]
fn toast_preferences_use_existing_strict_configuration() {
    let settings = Settings::parse("toast_placement = 'top-left'\ntoast_expiry_seconds = 12");
    assert!(settings.is_ok(), "{settings:?}");
    for text in [
        "toast_expiry_seconds = 0",
        "toast_expiry_seconds = 61",
        "toast_placement = 'middle'",
    ] {
        assert!(Settings::parse(text).is_err());
    }
}

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::{Duration, SystemTime};
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Action, Bindings, Controller},
    notifications::{Notifications, Severity},
    ui,
};

fn controller() -> Controller {
    Controller::new(
        App::new("owned-notifications".into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}
fn key(c: &mut Controller, code: KeyCode) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
    c.handle(
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        &g,
        Duration::ZERO,
    );
}
fn draw(app: &App, width: u16, height: u16) -> (String, ui::Geometry) {
    let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut g = ui::geometry(app, Rect::new(0, 0, width, height));
    t.draw(|f| g = ui::render(f, app, &Theme::new(colored_text::ColorLevel::NoColor)))
        .unwrap();
    (
        t.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect(),
        g,
    )
}
#[test]
fn expiry_deduplication_and_chronology_keep_all_session_evidence() {
    let mut n = Notifications::default();
    let now = SystemTime::UNIX_EPOCH;
    n.notify("failure".into(), now, Severity::Error);
    n.notify("failure".into(), now, Severity::Error);
    assert_eq!(n.history.len(), 1);
    assert_eq!(n.history[0].occurrences, 2);
    for i in 0..100 {
        n.notify(format!("info {i}"), now, Severity::Info);
    }
    assert_eq!(n.visible()[0], 100);
    assert_eq!(n.visible().len(), 101);
    n.advance(Duration::from_secs(5));
    assert!(n.visible().is_empty());
    assert_eq!(n.history.len(), 101);
    n.advance(Duration::ZERO);
    assert!(n.visible().is_empty());
    n.notify("failure".into(), now, Severity::Error);
    assert_eq!(n.history.len(), 102);
}
#[test]
fn expired_application_event_is_retrievable_without_a_filesystem_job() {
    let mut c = controller();
    let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
    c.dispatch(Action::Viewer, &g);
    assert_eq!(c.app.notifications.history.len(), 1);
    assert!(c.app.operations.history.is_empty());
    c.app.notifications.advance(Duration::from_secs(60));
    c.dispatch(Action::Notifications, &g);
    let (text, _) = draw(&c.app, 80, 24);
    assert!(text.contains("file viewer is not available yet"), "{text}");
    key(&mut c, KeyCode::Esc);
    assert!(!c.app.notifications.open);
    assert!(!c.quit);
}
#[test]
fn toast_geometry_and_status_stay_bounded_on_resize() {
    let mut c = controller();
    c.app.notifications.notify(
        "failure 界\u{202e}\n".repeat(50),
        c.app.now,
        Severity::Error,
    );
    for width in 20..=120 {
        for height in [0, 1, 2, 5, 12, 24] {
            let (_, g) = draw(&c.app, width, height);
            assert!(g.status.height <= 2);
            assert!(g.status.bottom() <= height);
            for toast in &g.toasts {
                assert!(toast.area.right() <= width && toast.area.bottom() <= height);
                assert!(!toast.area.intersects(g.footer));
                assert!(toast.area.height <= height * 4 / 5);
            }
        }
    }
}
#[test]
fn history_scroll_and_navigation_leave_panes_and_parent_surface_unchanged() {
    let mut c = controller();
    c.app
        .notifications
        .notify("first".into(), c.app.now, Severity::Info);
    c.app
        .notifications
        .notify("last ".repeat(1000), c.app.now, Severity::Warning);
    c.app.notifications.open = true;
    c.app.notifications.cursor = 1;
    c.app.operations.show_history = true;
    key(&mut c, KeyCode::Char('e'));
    key(&mut c, KeyCode::PageDown);
    assert!(c.app.notifications.scroll > 0);
    key(&mut c, KeyCode::Home);
    assert_eq!(c.app.notifications.cursor, 0);
    assert_eq!(c.app.notifications.scroll, 0);
    key(&mut c, KeyCode::End);
    assert_eq!(c.app.notifications.cursor, 1);
    key(&mut c, KeyCode::Esc);
    assert!(c.app.operations.show_history);
    assert_eq!(c.app.active, 0);
}

#[test]
fn four_placements_reflow_and_toast_clicks_do_not_reach_the_listing() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use terminal_file_manager::notifications::Placement;
    for (placement, left, top) in [
        (Placement::TopLeft, true, true),
        (Placement::TopRight, false, true),
        (Placement::BottomLeft, true, false),
        (Placement::BottomRight, false, false),
    ] {
        let mut c = controller();
        c.app.notifications.placement = placement;
        c.app.notify(Severity::Error, "Owned failure");
        let (text, g) = draw(&c.app, 120, 24);
        let toast = &g.toasts[0];
        assert!(text.contains("Owned failure"));
        assert_eq!(toast.area.x < 30, left);
        assert_eq!(toast.area.y < 10, top);
        c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: toast.area.x,
                row: toast.area.y,
                modifiers: KeyModifiers::NONE,
            }),
            &g,
            Duration::ZERO,
        );
        assert!(!c.app.notifications.open);
        assert!(c.app.notifications.visible().is_empty());
        assert_eq!(c.app.active, 0);
        assert!(c.app.panes[0].cursor.is_none());
    }
}

#[test]
fn foreground_toasts_cover_menus_and_long_history_remains_readable() {
    let mut c = controller();
    c.app.view_menu = Some(0);
    c.app.notify(
        Severity::Warning,
        "界e\u{301}\u{1b}[31m\u{202e}".repeat(100),
    );
    for width in [20, 40, 80, 120] {
        let (_, g) = draw(&c.app, width, 24);
        assert!(!g.toasts.is_empty());
        assert!(
            g.toasts
                .iter()
                .any(|toast| toast.area.intersects(g.menu.unwrap()))
        );
    }
    c.app.notifications.open = true;
    key(&mut c, KeyCode::Char('e'));
    let (text, g) = draw(&c.app, 40, 24);
    assert!(!text.contains('\u{1b}'));
    assert!(g.notification_history.unwrap().limit > 0);
    key(&mut c, KeyCode::PageDown);
    assert!(c.app.notifications.scroll > 0);
    key(&mut c, KeyCode::Esc);
    assert_eq!(c.app.view_menu, Some(0));
}

#[test]
fn accepted_listing_failures_retain_details_but_stale_results_and_navigation_do_not() {
    let mut c = controller();
    c.app
        .complete_listing(0, 1, Err("owned listing failure".into()));
    assert_eq!(c.app.notifications.history.len(), 1);
    c.app.complete_listing(0, 0, Err("stale".into()));
    assert_eq!(c.app.notifications.history.len(), 1);
    c.app
        .complete_listing(0, 1, Err("owned listing failure".into()));
    assert_eq!(c.app.notifications.history.len(), 1);
    let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
    for action in [Action::Down, Action::Up, Action::Parent, Action::Refresh] {
        c.dispatch(action, &g);
    }
    assert_eq!(c.app.notifications.history.len(), 1);
}

#[test]
fn configured_expiry_and_serialization_roundtrip_keep_safe_defaults() {
    for expiry in [1, 12, 60] {
        let settings = Settings::parse(&format!(
            "toast_expiry_seconds = {expiry}\ntoast_placement = 'top-right'"
        ))
        .unwrap();
        let settings = Settings::parse(&toml::to_string(&settings).unwrap()).unwrap();
        let mut app = App::new("owned-config".into());
        settings.apply(&mut app);
        app.notify(Severity::Info, "information");
        app.notifications.advance(Duration::from_secs(expiry - 1));
        assert_eq!(app.notifications.visible().len(), 1);
        app.notifications.advance(Duration::from_secs(expiry));
        assert!(app.notifications.visible().is_empty());
        assert_eq!(app.notifications.history.len(), 1);
    }
    assert_eq!(Settings::parse("").unwrap().toast_expiry_seconds, 5);
}

#[test]
fn semantic_severity_remains_textual_at_all_colour_depths() {
    use colored_text::ColorLevel::{Ansi16, Ansi256, NoColor, TrueColor};
    use terminal_file_manager::color::Role;
    let mut c = controller();
    c.app.notify(Severity::Error, "owned error");
    for level in [NoColor, Ansi16, Ansi256, TrueColor] {
        let theme = Theme::new(level);
        let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let g = ui::geometry(&c.app, Rect::new(0, 0, 80, 24));
        t.draw(|f| {
            ui::render(f, &c.app, &theme);
        })
        .unwrap();
        let area = g.toasts[0].area;
        let cell = &t.backend().buffer()[(area.x + 2, area.y + 1)];
        assert_eq!(cell.symbol(), "E");
        assert_eq!(
            cell.fg,
            theme
                .style(Role::Error)
                .fg
                .unwrap_or(ratatui::style::Color::Reset)
        );
        assert!(cell.modifier.contains(ratatui::style::Modifier::BOLD));
    }
}

#[test]
fn deduplicated_latest_event_and_history_mouse_controls_share_the_visible_geometry() {
    let mut c = controller();
    for text in ["repeated", "other", "repeated"] {
        c.app
            .notifications
            .notify(text.into(), c.app.now, Severity::Info);
    }
    assert_eq!(c.app.notifications.history.len(), 2);
    assert_eq!(c.app.notifications.visible()[0], 0);
    let (_, g) = draw(&c.app, 120, 24);
    let area = g
        .status_controls
        .iter()
        .find(|(_, action)| *action == Action::Notifications)
        .unwrap()
        .0;
    click(&mut c, &g, area);
    assert!(c.app.notifications.open);
    assert_eq!(c.app.notifications.cursor, 0);
    let (_, g) = draw(&c.app, 120, 24);
    let region = g
        .notification_history
        .as_ref()
        .unwrap()
        .shell
        .actions
        .iter()
        .find(|region| region.action.key == KeyCode::Down)
        .unwrap();
    click(&mut c, &g, region.area);
    assert_eq!(c.app.notifications.cursor, 1);
}

#[test]
fn reconciliation_notifications_keep_prior_events_without_repeating_identical_results() {
    let mut n = Notifications::default();
    let time = SystemTime::UNIX_EPOCH;
    n.record("Unknown".into(), time, Severity::Error, Some(7));
    n.record("Unknown".into(), time, Severity::Error, Some(7));
    assert_eq!(n.history.len(), 1);
    n.record("Reconciled".into(), time, Severity::Success, Some(7));
    assert_eq!(n.history.len(), 2);
    assert_eq!(n.visible(), [1]);
    assert_eq!(n.history[0].text, "Unknown");
    assert!(n.history.iter().all(|event| event.job == Some(7)));
}

fn click(c: &mut Controller, g: &ui::Geometry, area: Rect) {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }),
        g,
        Duration::ZERO,
    );
}

#[test]
fn current_error_precedes_running_jobs_and_decorative_history_on_narrow_status() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller();
    c.register_operation(
        1,
        terminal_file_manager::operations::Plan::mkdir(root.path().join("pending")),
        Default::default(),
        None,
    );
    c.app.notify(Severity::Error, "Important failure");
    c.app.notifications.dismiss_toasts();
    let (text, g) = draw(&c.app, 20, 24);
    assert!(text.contains("Important failure"), "{text}");
    assert_eq!(g.status_controls[0].1, Action::Notifications);
    assert_eq!(g.status.height, 1);
}
