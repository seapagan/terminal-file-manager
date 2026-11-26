//! Focused inputs own editing keys, including the global Delete aliases.
use crossterm::event::{Event, KeyEvent};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use terminal_file_manager::{
    app::App,
    input::{Bindings, Controller, chord},
    operations::Dialog,
    ui,
};
fn controller() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("selected"), "owned").unwrap();
    let mut app = App::new(root.path().into());
    let snapshot =
        terminal_file_manager::fs::list(root.path(), &Default::default(), &Default::default())
            .unwrap();
    app.complete_listing(0, 1, Ok(snapshot));
    app.panes[0].move_cursor(1, 20);
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        ),
    )
}
fn event(c: &mut Controller, event: Event) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 32));
    assert!(c.handle(event, &g, Default::default()).is_empty());
}
fn key(c: &mut Controller, text: &str) {
    let (code, modifiers) = chord(text).unwrap();
    event(c, Event::Key(KeyEvent::new(code, modifiers)));
}
fn edits(c: &mut Controller) {
    event(c, Event::Paste("abcdef".into()));
    for text in [
        "Home",
        "Right",
        "Delete",
        "X",
        "Insert",
        "Y",
        "End",
        "Backspace",
        "Left",
    ] {
        key(c, text);
    }
    event(c, Event::Paste("Z".into()));
    key(c, "Home");
    key(c, "Delete");
    assert!(c.app.operations.outbox.iter().all(|intent| !matches!(
        intent,
        terminal_file_manager::operations::Intent::Submit(..)
    )));
}
#[test]
fn pattern_filter_and_operation_fields_insert_delete_and_overwrite_at_cursor() {
    for shortcut in ["/", "Alt+b", "F7", "Alt+n"] {
        let (_root, mut c) = controller();
        key(&mut c, shortcut);
        edits(&mut c);
        let text = match shortcut {
            "/" => c.app.panes[0].filter.as_str(),
            "Alt+b" => c.app.pattern.as_ref().unwrap().expression(),
            _ => {
                let Some(Dialog::Operation(d)) = &c.app.operations.dialog else {
                    panic!("operation form")
                };
                &d.text
            }
        };
        assert_eq!(text, "XYdZ", "{shortcut}");
    }
}
#[test]
fn profile_name_and_help_search_use_the_same_cursor_editor() {
    let (_root, mut c) = controller();
    c.app.profile_menu = Some(0);
    c.app.profile_name = Some(String::new());
    edits(&mut c);
    assert_eq!(c.app.profile_name.as_deref(), Some("XYdZ"));
    c.app.profile_name = None;
    c.app.profile_menu = None;
    key(&mut c, "?");
    key(&mut c, "/");
    edits(&mut c);
    assert_eq!(c.app.help_picker.as_ref().unwrap().query, "XYdZ");
}
#[test]
fn real_cursor_tracks_unicode_without_displacing_the_value() {
    let (_root, mut c) = controller();
    key(&mut c, "Alt+b");
    event(
        &mut c,
        Event::Paste(format!("{}👩‍💻e\u{301}Z", "界".repeat(50))),
    );
    key(&mut c, "Left");
    key(&mut c, "Backspace");
    key(&mut c, "Insert");
    assert!(
        c.app
            .pattern
            .as_ref()
            .unwrap()
            .expression()
            .ends_with("👩‍💻Z")
    );
    for width in [24, 40, 100] {
        let mut t = Terminal::new(TestBackend::new(width, 32)).unwrap();
        t.draw(|f| {
            ui::render(
                f,
                &c.app,
                &terminal_file_manager::color::Theme::new(colored_text::ColorLevel::NoColor),
            );
        })
        .unwrap();
        let text: String = t
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!text.contains('▏') && !text.contains('▐'), "{width}");
        assert!(t.backend().cursor_visible(), "{width}");
        let cursor = t.backend().cursor_position();
        assert_eq!(t.backend().buffer()[cursor].symbol(), "Z", "{width}");
    }
}

#[test]
fn transfer_fields_keep_independent_cursors_and_native_target_until_an_edit() {
    let (_root, mut c) = controller();
    key(&mut c, "F5");
    key(&mut c, "Left");
    let Some(Dialog::Operation(d)) = &c.app.operations.dialog else {
        panic!("copy form")
    };
    assert!(!d.edited);
    key(&mut c, "Ctrl+a");
    edits(&mut c);
    key(&mut c, "Tab");
    edits(&mut c);
    let Some(Dialog::Operation(d)) = &c.app.operations.dialog else {
        panic!("copy form")
    };
    assert_eq!(d.text, "XYdZ");
    assert_eq!(d.incoming, "XYdZ");
    assert!(d.edited);
    let mut t = Terminal::new(TestBackend::new(120, 32)).unwrap();
    draw(&mut t, &c);
    assert!(t.backend().cursor_visible());
    assert_eq!(
        t.backend().buffer()[t.backend().cursor_position()].symbol(),
        "X"
    );
}

#[test]
fn incident_acknowledgement_allows_cursor_correction_but_never_paste_approval() {
    let (_root, mut c) = controller();
    c.app.operations.dialog = Some(Dialog::IncidentExit {
        save: false,
        typed: String::new(),
    });
    event(&mut c, Event::Paste("ACKNOWLEDGE".into()));
    let Some(Dialog::IncidentExit { typed, .. }) = &c.app.operations.dialog else {
        panic!("incident form")
    };
    assert!(typed.is_empty());
    for text in ["A", "X", "K", "Left", "Left", "Delete", "C"] {
        key(&mut c, text);
    }
    let Some(Dialog::IncidentExit { typed, .. }) = &c.app.operations.dialog else {
        panic!("incident form")
    };
    assert_eq!(typed, "ACK");
    assert!(!c.quit);
    let mut t = Terminal::new(TestBackend::new(120, 32)).unwrap();
    draw(&mut t, &c);
    assert!(t.backend().cursor_visible());
    assert_eq!(
        t.backend().buffer()[t.backend().cursor_position()].symbol(),
        "K"
    );
}

#[test]
fn moving_and_changing_cursor_shape_preserve_buffers_across_all_fields() {
    for shortcut in ["/", "Alt+b", "Alt+n", "profile", "search"] {
        let (_root, mut c) = controller();
        match shortcut {
            "profile" => {
                c.app.profile_menu = Some(0);
                c.app.profile_name = Some(String::new());
            }
            "search" => {
                key(&mut c, "?");
                key(&mut c, "/");
            }
            _ => key(&mut c, shortcut),
        }
        event(&mut c, Event::Paste("abc".into()));
        key(&mut c, "Home");
        let mut t = Terminal::new(TestBackend::new(120, 32)).unwrap();
        draw(&mut t, &c);
        let baseline = t.backend().buffer().clone();
        assert!(t.backend().cursor_visible(), "{shortcut}");
        assert_eq!(baseline[t.backend().cursor_position()].symbol(), "a");
        key(&mut c, "Right");
        draw(&mut t, &c);
        assert_eq!(t.backend().buffer(), &baseline, "{shortcut}");
        assert_eq!(baseline[t.backend().cursor_position()].symbol(), "b");
        key(&mut c, "Insert");
        draw(&mut t, &c);
        assert_eq!(t.backend().buffer(), &baseline, "{shortcut}");
        assert_eq!(baseline[t.backend().cursor_position()].symbol(), "b");
    }
}
fn draw(t: &mut Terminal<TestBackend>, c: &Controller) {
    t.draw(|f| {
        ui::render(
            f,
            &c.app,
            &terminal_file_manager::color::Theme::new(colored_text::ColorLevel::NoColor),
        );
    })
    .unwrap();
}

#[test]
fn context_help_and_non_editing_dialogs_hide_the_underlying_field_cursor() {
    let (_root, mut c) = controller();
    key(&mut c, "Alt+n");
    event(&mut c, Event::Paste("abc".into()));
    key(&mut c, "Home");
    key(&mut c, "Right");
    let mut t = Terminal::new(TestBackend::new(120, 32)).unwrap();
    draw(&mut t, &c);
    let position = t.backend().cursor_position();
    key(&mut c, "F1");
    assert!(
        ui::geometry(&c.app, Rect::new(0, 0, 120, 32))
            .context_help
            .is_some()
    );
    draw(&mut t, &c);
    assert!(!t.backend().cursor_visible());
    key(&mut c, "Esc");
    draw(&mut t, &c);
    assert!(t.backend().cursor_visible());
    assert_eq!(t.backend().cursor_position(), position);
    c.app.operations.dialog = None;
    c.app.operations.show_history = true;
    draw(&mut t, &c);
    assert!(!t.backend().cursor_visible());
}
