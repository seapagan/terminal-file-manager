//! Diagnostics must leave browsing and the underlying rendered styles intact.
use colored_text::ColorLevel;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
};
use std::{sync::atomic::AtomicBool, time::Duration};
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs::{IgnoreContext, list},
    input::{Action, Bindings, Controller},
    ui,
};

fn controller() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    for name in ["alpha", "beta"] {
        std::fs::create_dir(root.path().join(name)).unwrap();
    }
    let mut app = App::new(root.path().into());
    app.dual = true;
    for id in 0..2 {
        app.complete_listing(
            id,
            1,
            Ok(list(
                root.path(),
                &IgnoreContext::isolated(),
                &AtomicBool::new(false),
            )
            .unwrap()),
        );
    }
    let controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    (root, controller)
}

fn draw(c: &Controller, level: ColorLevel) -> (Buffer, ui::Geometry) {
    draw_at(c, level, 160, 40)
}

fn draw_at(c: &Controller, level: ColorLevel, width: u16, height: u16) -> (Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut geometry = ui::geometry(&c.app, terminal.backend().buffer().area);
    terminal
        .draw(|frame| geometry = ui::render(frame, &c.app, &Theme::new(level)))
        .unwrap();
    (terminal.backend().buffer().clone(), geometry)
}

fn key(c: &mut Controller, geometry: &ui::Geometry, code: KeyCode) {
    c.handle(
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        geometry,
        Duration::ZERO,
    );
}

fn text(buffer: &Buffer) -> String {
    buffer.content().iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn f12_toggles_diagnostics_without_changing_the_underlying_browser() {
    let (_root, mut c) = controller();
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        assert_toggle(&mut c, level);
    }
}

#[test]
fn diagnostics_groups_information_and_styles_headings_at_the_applied_depth() {
    let (_root, mut c) = controller();
    c.app.diagnostics = true;
    for (level, heading_colour) in [
        (ColorLevel::NoColor, Color::Reset),
        (ColorLevel::Ansi16, Color::Cyan),
        (ColorLevel::Ansi256, Color::Indexed(30)),
        (ColorLevel::TrueColor, Color::Rgb(35, 140, 150)),
    ] {
        let (buffer, geometry) = draw(&c, level);
        let area = geometry.diagnostics.unwrap();
        let heading = (area.y + 1..area.bottom())
            .find(|y| {
                (area.x + 1..area.right() - 1)
                    .map(|x| buffer[(x, *y)].symbol())
                    .collect::<String>()
                    .contains("CAPABILITY")
            })
            .expect("a separate capability heading");
        let cell = &buffer[(area.x + 1, heading)];
        assert_eq!(cell.fg, heading_colour);
        assert!(cell.modifier.contains(Modifier::BOLD));
        let visible = text(&buffer);
        assert!(visible.contains("LAYOUT"));
        assert!(visible.contains("INTENDED STYLES"));
        assert!(visible.contains("fg=inherit bg=inherit"));
    }
}

fn assert_toggle(c: &mut Controller, level: ColorLevel) {
    let (before, geometry) = draw(c, level);
    key(c, &geometry, KeyCode::F(12));
    let (open, geometry) = draw(c, level);
    assert!(text(&open).contains("Diagnostics"));
    assert!(text(&open).contains(&format!("Applied ColorLevel: {level:?}")));
    let overlay = geometry.diagnostics.unwrap();
    for (index, cell) in before.content().iter().enumerate() {
        let point = ((index % 160) as u16, (index / 160) as u16);
        if !overlay.contains(point.into()) {
            assert_eq!(open[point], *cell);
        }
    }
    assert!(!c.preferences_changed);
    key(c, &geometry, KeyCode::F(12));
    assert_eq!(draw(c, level).0, before);
}

fn mouse(c: &mut Controller, geometry: &ui::Geometry, kind: MouseEventKind, x: u16, y: u16) {
    assert!(
        c.handle(
            Event::Mouse(MouseEvent {
                kind,
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE
            }),
            geometry,
            Duration::ZERO
        )
        .is_empty()
    );
}

#[test]
fn diagnostics_consumes_its_mouse_events_but_not_adjacent_rows() {
    let (_root, mut c) = controller();
    c.app.diagnostics = true;
    let (_, geometry) = draw(&c, ColorLevel::TrueColor);
    let area = geometry.diagnostics.unwrap();
    let cursor = c.app.panes[1].cursor.clone();
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::ScrollDown,
    ] {
        mouse(&mut c, &geometry, kind, area.x, 3);
        assert_eq!(c.app.active, 0);
        assert_eq!(c.app.panes[1].cursor, cursor);
        assert!(!c.preferences_changed);
    }
    mouse(
        &mut c,
        &geometry,
        MouseEventKind::Down(MouseButton::Left),
        area.x - 1,
        3,
    );
    assert_eq!(c.app.active, 1);
    assert!(c.app.panes[1].cursor.as_ref().unwrap().ends_with("beta"));
    mouse(
        &mut c,
        &geometry,
        MouseEventKind::Down(MouseButton::Left),
        area.right() - 1,
        area.bottom(),
    );
    assert_eq!(c.app.active, 1);
}

#[test]
fn diagnostics_stays_bounded_and_scrolls_after_excess_scroll_and_resize() {
    let (_root, mut c) = controller();
    c.app.diagnostics = true;
    for (width, height) in [(0, 0), (1, 1), (2, 2), (20, 5), (79, 12), (160, 40)] {
        let (_, geometry) = draw_at(&c, ColorLevel::NoColor, width, height);
        let area = geometry.diagnostics.unwrap();
        assert_eq!(area.right(), width);
        assert_eq!(area.y, 0);
        assert!(area.bottom() <= height);
        c.app.diagnostics_scroll = u16::MAX;
        mouse(
            &mut c,
            &geometry,
            MouseEventKind::ScrollDown,
            area.x,
            area.y,
        );
        if !area.is_empty() {
            assert_eq!(c.app.diagnostics_scroll, geometry.diagnostics_scroll_limit);
            mouse(&mut c, &geometry, MouseEventKind::ScrollUp, area.x, area.y);
            assert_eq!(
                c.app.diagnostics_scroll,
                geometry.diagnostics_scroll_limit.saturating_sub(1)
            );
        }
        c.handle(Event::Resize(width, height), &geometry, Duration::ZERO);
        draw_at(&c, ColorLevel::NoColor, width, height);
    }
}

#[test]
fn diagnostic_keys_work_in_filter_and_menus_without_taking_their_navigation() {
    let (_root, mut c) = controller();
    for action in [
        Action::Filter,
        Action::ViewOptions,
        Action::ViewProfiles,
        Action::Help,
        Action::Metadata,
    ] {
        let (_, geometry) = draw(&c, ColorLevel::Ansi256);
        c.dispatch(action, &geometry);
        key(&mut c, &geometry, KeyCode::F(12));
        assert!(c.app.diagnostics);
        let (_, geometry) = draw(&c, ColorLevel::Ansi256);
        c.handle(
            Event::Key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::ALT)),
            &geometry,
            Duration::ZERO,
        );
        assert!(c.app.diagnostics_scroll > 0);
        key(&mut c, &geometry, KeyCode::F(12));
        assert!(!c.app.diagnostics);
        key(&mut c, &geometry, KeyCode::Esc);
    }
    let (_, geometry) = draw(&c, ColorLevel::Ansi256);
    c.app.diagnostics = true;
    key(&mut c, &geometry, KeyCode::Down);
    assert!(c.app.panes[0].cursor.as_ref().unwrap().ends_with("beta"));
    key(&mut c, &geometry, KeyCode::Tab);
    assert_eq!(c.app.active, 1);
    c.dispatch(Action::ViewOptions, &geometry);
    key(&mut c, &geometry, KeyCode::Enter);
    assert!(!c.app.panes[1].human_sizes);
    assert!(c.app.diagnostics);
}

#[test]
fn explicit_f12_binding_keeps_its_existing_action() {
    let (_root, c) = controller();
    let mut c = Controller::new(
        c.app,
        Bindings::new(&[("F12".into(), Action::TogglePanes)].into()).unwrap(),
        Default::default(),
    );
    let (_, geometry) = draw(&c, ColorLevel::NoColor);
    key(&mut c, &geometry, KeyCode::F(12));
    assert!(!c.app.dual);
    assert!(!c.app.diagnostics);
}

#[test]
fn profile_selection_and_key_release_keep_diagnostics_independent() {
    let (_root, mut c) = controller();
    c.app.diagnostics = true;
    let (_, geometry) = draw(&c, ColorLevel::TrueColor);
    c.dispatch(Action::ViewProfiles, &geometry);
    key(&mut c, &geometry, KeyCode::Down);
    key(&mut c, &geometry, KeyCode::Enter);
    assert_eq!(c.app.panes[0].profile.as_deref(), Some("Detailed"));
    assert!(c.app.sidebar);
    assert!(c.app.diagnostics);
    let release = KeyEvent::new_with_kind(
        KeyCode::F(12),
        KeyModifiers::NONE,
        crossterm::event::KeyEventKind::Release,
    );
    c.handle(Event::Key(release), &geometry, Duration::ZERO);
    assert!(c.app.diagnostics);
    assert!(!c.quit);
}
