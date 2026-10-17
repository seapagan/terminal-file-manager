//! Adjustable layout never replaces independent pane state.
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    input::{Action, Bindings, Controller},
    settings::Settings,
    ui,
};

#[test]
fn divider_drag_preserves_ratio_through_resize_and_release() {
    let mut controller = controller();
    controller.app.split_percent = 80;
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 160, 24));
    let divider = geometry.divider.unwrap();
    let mouse = |kind, column| {
        Event::Mouse(MouseEvent {
            kind,
            column,
            row: 5,
            modifiers: KeyModifiers::NONE,
        })
    };
    controller.handle(
        mouse(MouseEventKind::Down(MouseButton::Left), divider.x),
        &geometry,
        Duration::ZERO,
    );
    controller.handle(
        mouse(MouseEventKind::Drag(MouseButton::Left), 120),
        &geometry,
        Duration::ZERO,
    );
    assert_eq!(controller.app.split_percent, 75);
    assert_eq!(controller.preferences.split_percent, 75);
    let narrow = ui::geometry(&controller.app, Rect::new(0, 0, 80, 12));
    assert!(narrow.panes.iter().all(|area| area.unwrap().width >= 24));
    assert_eq!(controller.app.split_percent, 75);
    assert!(
        ui::geometry(&controller.app, Rect::new(0, 0, 40, 12))
            .divider
            .is_none()
    );
    controller.handle(
        mouse(MouseEventKind::Up(MouseButton::Left), 1),
        &narrow,
        Duration::ZERO,
    );
    controller.handle(
        mouse(MouseEventKind::Drag(MouseButton::Left), 1),
        &narrow,
        Duration::ZERO,
    );
    assert_eq!(controller.app.split_percent, 75);
    assert!(Settings::parse("split_percent=19").is_err());
}

fn controller() -> Controller {
    let mut app = App::new(".".into());
    app.dual = true;
    Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    )
}

#[test]
fn keyboard_divider_adjustment_and_config_enforce_bounds() {
    let mut controller = controller();
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 160, 24));
    for _ in 0..20 {
        controller.dispatch(Action::SplitLeft, &geometry);
    }
    assert_eq!(controller.app.split_percent, 20);
    for _ in 0..20 {
        controller.dispatch(Action::SplitRight, &geometry);
    }
    assert_eq!(controller.app.split_percent, 80);
    assert!(Settings::parse("split_percent=81").is_err());
}

#[test]
fn resize_cancels_stale_drag_and_keyboard_adjustment_still_works() {
    let mut controller = controller();
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 100, 24));
    let event = |kind, column| {
        Event::Mouse(MouseEvent {
            kind,
            column,
            row: 5,
            modifiers: KeyModifiers::NONE,
        })
    };
    controller.handle(
        event(
            MouseEventKind::Down(MouseButton::Left),
            geometry.divider.unwrap().x,
        ),
        &geometry,
        Duration::ZERO,
    );
    let narrow = ui::geometry(&controller.app, Rect::new(0, 0, 80, 12));
    controller.handle(Event::Resize(80, 12), &narrow, Duration::ZERO);
    controller.handle(
        event(MouseEventKind::Drag(MouseButton::Left), 10),
        &narrow,
        Duration::ZERO,
    );
    assert_eq!(controller.app.split_percent, 50);
    controller.dispatch(Action::SplitRight, &narrow);
    assert_eq!(controller.app.split_percent, 55);
}
