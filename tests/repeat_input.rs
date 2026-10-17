//! Repeated one-shot keys cannot cross a menu boundary into the listing.
use crossterm::event::{Event, KeyEvent, KeyEventKind, KeyEventKind::*};
use ratatui::layout::Rect;
use std::{sync::atomic::AtomicBool, time::Duration};
use terminal_file_manager::{
    app::App,
    fs,
    input::{Bindings, Controller, chord},
    ui,
};

fn loaded() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    for name in ["alpha", "beta"] {
        std::fs::write(root.path().join(name), "data").unwrap();
    }
    let mut app = App::new(root.path().into());
    app.complete_listing(
        0,
        1,
        Ok(fs::list(
            root.path(),
            &fs::IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        ),
    )
}

fn key(c: &mut Controller, text: &str, kind: KeyEventKind) {
    let (code, modifiers) = chord(text).unwrap();
    let mut g = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 24)).unwrap();
    terminal
        .draw(|frame| {
            g = ui::render(
                frame,
                &c.app,
                &terminal_file_manager::color::Theme::new(colored_text::ColorLevel::NoColor),
            );
        })
        .unwrap();
    assert!(
        c.handle(
            Event::Key(KeyEvent::new_with_kind(code, modifiers, kind)),
            &g,
            Duration::ZERO
        )
        .is_empty()
    );
}

#[test]
fn held_space_cannot_apply_a_profile_close_its_menu_or_mark_files() {
    let (_root, mut c) = loaded();
    key(&mut c, "Alt+v", Press);
    key(&mut c, "Space", Press);
    let apply = c.app.profile_menu;
    assert_eq!(apply, Some(c.app.profile_names.len()));
    for kind in [Repeat, Repeat, Release] {
        key(&mut c, "Space", kind);
        assert_eq!(c.app.profile_menu, apply);
        assert!(!c.preferences_changed && c.app.panes[0].marks.is_empty());
    }
    key(&mut c, "Space", Press);
    assert!(c.app.profile_menu.is_none() && c.preferences_changed);
    for kind in [Repeat, Release] {
        key(&mut c, "Space", kind);
        assert!(c.app.panes[0].marks.is_empty());
    }
    key(&mut c, "Space", Press);
    assert_eq!(c.app.panes[0].marks.len(), 1);
}

#[test]
fn view_and_diagnostics_toggle_once_but_menu_navigation_repeats() {
    let (_root, mut c) = loaded();
    key(&mut c, "V", Press);
    key(&mut c, "Space", Press);
    assert!(!c.app.panes[0].human_sizes);
    for kind in [Repeat, Release] {
        key(&mut c, "Space", kind);
        assert!(!c.app.panes[0].human_sizes);
    }
    key(&mut c, "Down", Repeat);
    assert_eq!(c.app.view_menu, Some(1));
    key(&mut c, "Down", Release);
    assert_eq!(c.app.view_menu, Some(1));
    key(&mut c, "F12", Press);
    key(&mut c, "Alt+PageDown", Repeat);
    assert_eq!(c.app.diagnostics_scroll, 1);
    key(&mut c, "Alt+PageDown", Release);
    assert_eq!(c.app.diagnostics_scroll, 1);
    key(&mut c, "Alt+PageUp", Repeat);
    assert_eq!(c.app.diagnostics_scroll, 0);
    for kind in [Repeat, Release] {
        key(&mut c, "F12", kind);
        assert!(c.app.diagnostics);
    }
    key(&mut c, "F12", Press);
    assert!(!c.app.diagnostics);
}

#[test]
fn profile_text_and_browsing_navigation_repeat_without_repeating_confirmation() {
    let (_root, mut c) = loaded();
    key(&mut c, "Down", Repeat);
    assert_eq!(c.app.panes[0].current().unwrap().name, "beta");
    key(&mut c, "Alt+v", Press);
    key(&mut c, "Down", Repeat);
    assert_eq!(c.app.profile_menu, Some(1));
    key(&mut c, "End", Press);
    key(&mut c, "Enter", Press);
    let before = c.app.profile_name.clone().unwrap();
    key(&mut c, "x", Repeat);
    assert_eq!(c.app.profile_name.as_ref().unwrap(), &format!("{before}x"));
    key(&mut c, "Enter", Repeat);
    assert!(c.app.profile_name.is_some() && !c.preferences_changed);
    key(&mut c, "Esc", Press);
    key(&mut c, "Esc", Press);
    key(&mut c, "V", Repeat);
    assert!(c.app.view_menu.is_none());
}
