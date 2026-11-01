//! A Help session shares ranked results across geometry, rendering and input.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::Theme,
    input::{Action, Bindings, Controller, HelpTarget},
    ui,
};

fn controller() -> Controller {
    let mut c = Controller::new(
        App::new("owned-search-cache".into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    c.app.help = true;
    c.app.help_picker = Some(Default::default());
    c
}
fn geometry(c: &Controller) -> ui::Geometry {
    ui::geometry(&c.app, Rect::new(0, 0, 120, 32))
}
#[test]
fn unchanged_query_reuses_hits_through_redraw_and_keyboard_selection() {
    let mut c = controller();
    let first = geometry(&c).help.unwrap().picker.unwrap();
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    let mut rendered = None;
    terminal
        .draw(|f| {
            rendered = Some(ui::render(f, &c.app, &Theme::new(ColorLevel::NoColor)));
        })
        .unwrap();
    let g = rendered.unwrap();
    let second = g.help.as_ref().unwrap().picker.as_ref().unwrap();
    assert!(std::ptr::eq(first.hits.as_ptr(), second.hits.as_ptr()));
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        &g,
        Duration::ZERO,
    );
    assert_eq!(c.app.help_picker.as_ref().unwrap().selected, 1);
    let g = geometry(&c);
    let third = g.help.as_ref().unwrap().picker.as_ref().unwrap();
    assert!(std::ptr::eq(first.hits.as_ptr(), third.hits.as_ptr()));
    let target = third.hits[1].target;
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &g,
        Duration::ZERO,
    );
    assert_eq!(c.app.help_landing.unwrap().0, target);
    assert!(c.app.help && c.app.help_picker.is_none());
    assert!(c.app.operations.outbox.is_empty());
}
#[test]
fn query_and_effective_bindings_invalidate_results_without_stale_targets() {
    let mut c = controller();
    let first = geometry(&c).help.unwrap().picker.unwrap();
    c.app.help_picker.as_mut().unwrap().query = "rename".into();
    let query_change = geometry(&c).help.unwrap().picker.unwrap();
    assert!(!std::ptr::eq(
        first.hits.as_ptr(),
        query_change.hits.as_ptr()
    ));
    assert_eq!(
        query_change.hits[0].target,
        HelpTarget::Action(Action::Rename)
    );
    c.app.help_picker.as_mut().unwrap().query = "F11".into();
    c.app.help_rows = Bindings::new(&[("F11".into(), Action::Rename)].into())
        .unwrap()
        .help_entries();
    let renamed = geometry(&c).help.unwrap().picker.unwrap();
    assert!(!std::ptr::eq(first.hits.as_ptr(), renamed.hits.as_ptr()));
    assert_eq!(renamed.hits.len(), 1);
    assert_eq!(renamed.hits[0].target, HelpTarget::Action(Action::Rename));
    c.app.help_rows = Bindings::new(&[("F11".into(), Action::Copy)].into())
        .unwrap()
        .help_entries();
    let g = geometry(&c);
    let copied = g.help.as_ref().unwrap().picker.as_ref().unwrap();
    assert_eq!(copied.hits.len(), 1);
    assert_eq!(copied.hits[0].target, HelpTarget::Action(Action::Copy));
    assert!(copied.hits[0].keys.contains("F11"));
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        &g,
        Duration::ZERO,
    );
    assert_eq!(
        c.app.help_landing.unwrap().0,
        HelpTarget::Action(Action::Copy)
    );
    assert!(c.app.operations.outbox.is_empty());
}
