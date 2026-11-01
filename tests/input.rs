//! Keyboard, filter and geometry-based mouse routing use the same actions.
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::{fs, sync::atomic::AtomicBool, time::Duration};
use terminal_file_manager::{
    app::App,
    fs::{IgnoreContext, list},
    input::{Bindings, Controller},
    ui,
};

fn controller() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("alpha"), "one").unwrap();
    fs::write(root.path().join("beta"), "two").unwrap();
    let mut app = App::new(root.path().to_path_buf());
    let snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    for pane in 0..2 {
        app.complete_listing(pane, 1, Ok(snapshot.clone()));
    }
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        ),
    )
}

fn key(controller: &mut Controller, chord: &str) {
    let (code, modifiers) = terminal_file_manager::input::chord(chord).unwrap();
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    controller.handle(
        Event::Key(KeyEvent::new(code, modifiers)),
        &geometry,
        Duration::ZERO,
    );
}

#[test]
fn filter_accept_cancel_paste_and_quit_do_not_confuse_actions_with_text() {
    let (_root, mut controller) = controller();
    key(&mut controller, "/");
    key(&mut controller, "q");
    assert!(!controller.quit);
    assert!(controller.app.panes[0].visible.is_empty());
    key(&mut controller, "Esc");
    assert_eq!(controller.app.panes[0].filter, "");
    key(&mut controller, "/");
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    controller.handle(Event::Paste("bet\n".into()), &geometry, Duration::ZERO);
    key(&mut controller, "Enter");
    assert_eq!(controller.app.panes[0].current().unwrap().name, "beta");
}

#[test]
fn files_explain_viewer_unavailability_and_copy_opens_a_frozen_dialog() {
    let (_root, mut controller) = controller();
    key(&mut controller, "Enter");
    assert!(controller.app.status.contains("M6"));
    key(&mut controller, "F5");
    assert!(controller.app.operations.dialog.is_some());
    assert!(controller.app.operations.outbox.is_empty());
    key(&mut controller, "Ctrl+c");
    assert!(controller.quit);
}

#[test]
fn alt_p_and_both_tab_encodings_share_the_two_pane_focus_action() {
    let (_root, mut controller) = controller();
    key(&mut controller, "Ctrl+p");
    assert!(!controller.app.dual);
    key(&mut controller, "Alt+p");
    assert!(controller.app.dual);
    key(&mut controller, "Tab");
    assert_eq!(controller.app.active, 1);
    key(&mut controller, "Shift+Tab");
    assert_eq!(controller.app.active, 0);
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    controller.handle(
        Event::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
        &geometry,
        Duration::ZERO,
    );
    assert_eq!(controller.app.active, 1);
}

#[test]
fn preferred_metadata_presentation_keeps_both_direct_shortcuts() {
    let (_root, mut controller) = controller();
    controller.preferences =
        terminal_file_manager::settings::Settings::parse("metadata_presentation = 'sidebar'")
            .unwrap();
    key(&mut controller, "F2");
    assert!(controller.app.sidebar);
    assert!(!controller.app.popup);
    key(&mut controller, "Alt+Enter");
    assert!(controller.app.popup);
    key(&mut controller, "Esc");
    key(&mut controller, "Alt+i");
    assert!(!controller.app.sidebar);
}

#[test]
fn floating_metadata_close_keys_preserve_sidebar_and_modal_focus() {
    use terminal_file_manager::settings::MetadataPresentation;
    for preference in [MetadataPresentation::Sidebar, MetadataPresentation::Popup] {
        for sidebar in [false, true] {
            for close in ["F2", "Alt+Enter", "Esc"] {
                let (_root, mut c) = controller();
                c.preferences.metadata_presentation = preference;
                c.app.sidebar = sidebar;
                c.app.panes[0].sidebar = sidebar;
                key(&mut c, "Alt+Enter");
                assert!(c.app.popup);
                let cursor = c.app.panes[0].cursor.clone();
                for blocked in ["Tab", "Space", "F5", "F8", "Alt+i"] {
                    key(&mut c, blocked);
                    assert!(c.app.popup);
                    assert_eq!(c.app.active, 0);
                    assert_eq!(c.app.panes[0].cursor, cursor);
                    assert!(c.app.panes[0].marks.is_empty());
                    assert!(c.app.operations.dialog.is_none());
                    assert!(c.app.operations.outbox.is_empty());
                }
                key(&mut c, close);
                assert!(!c.app.popup, "{preference:?}: {close}");
                assert_eq!(c.app.sidebar, sidebar);
                assert_eq!(c.app.panes[0].sidebar, sidebar);
                assert!(!c.preferences_changed);
            }
        }
    }
}

#[test]
fn help_lists_effective_bindings_and_centres_the_action_table() {
    use colored_text::ColorLevel;
    use ratatui::{Terminal, backend::TestBackend};
    let (_root, controller) = controller();
    let settings = terminal_file_manager::settings::Settings::parse(
        "[bindings]\n'Ctrl+p'='quit'\n'F12'='toggle-panes'",
    )
    .unwrap();
    let mut controller = Controller::new(
        controller.app,
        Bindings::new(&settings.bindings).unwrap(),
        settings,
    );
    key(&mut controller, "?");
    let mut terminal = Terminal::new(TestBackend::new(160, 60)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(
                frame,
                &controller.app,
                &terminal_file_manager::color::Theme::new(ColorLevel::NoColor),
            );
        })
        .unwrap();
    let rows = terminal
        .backend()
        .buffer()
        .content()
        .chunks(160)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>();
    let pane_row = rows
        .iter()
        .find(|row| row.contains("Toggle one/two panes"))
        .unwrap();
    assert!(pane_row.contains("F12"));
    assert!(!pane_row.contains("Ctrl+P"));
    let title = rows.iter().position(|row| row.contains("Help —")).unwrap();
    assert!(title > 0);
    assert!(rows[title].find('┌').unwrap() > 0);
}

#[test]
fn help_scrolling_reaches_last_action_without_changing_pane_policy() {
    use colored_text::ColorLevel;
    use ratatui::{Terminal, backend::TestBackend};
    let (_root, mut controller) = controller();
    key(&mut controller, "?");
    key(&mut controller, "h");
    assert!(!controller.app.panes[0].show_hidden);
    for (width, height) in [(1, 1), (12, 5), (80, 10)] {
        let geometry = ui::geometry(
            &controller.app,
            ratatui::layout::Rect::new(0, 0, width, height),
        );
        controller.dispatch(terminal_file_manager::input::Action::End, &geometry);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                ui::render(
                    frame,
                    &controller.app,
                    &terminal_file_manager::color::Theme::new(ColorLevel::NoColor),
                );
            })
            .unwrap();
        if width == 80 {
            let text = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            let last = controller
                .app
                .help_rows
                .iter()
                .rev()
                .find(|entry| {
                    entry.action.help_section()
                        == terminal_file_manager::input::HelpSection::Application
                })
                .unwrap();
            assert!(text.contains(last.description));
        }
    }
    key(&mut controller, "Esc");
    assert!(!controller.app.help);
}

#[test]
fn wheel_does_not_change_focus_and_double_click_never_launches_files() {
    let (_root, mut controller) = controller();
    controller.app.dual = true;
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    let event = |kind| {
        Event::Mouse(MouseEvent {
            kind,
            column: 65,
            row: 2,
            modifiers: KeyModifiers::NONE,
        })
    };
    controller.handle(event(MouseEventKind::ScrollDown), &geometry, Duration::ZERO);
    assert_eq!(controller.app.active, 0);
    assert_eq!(controller.app.panes[1].current().unwrap().name, "beta");
    controller.handle(
        event(MouseEventKind::Down(MouseButton::Left)),
        &geometry,
        Duration::from_millis(100),
    );
    assert_eq!(controller.app.active, 1);
    controller.handle(
        event(MouseEventKind::Down(MouseButton::Left)),
        &geometry,
        Duration::from_millis(200),
    );
    assert!(controller.app.status.contains("M6"));
}

#[test]
fn shifted_printable_help_chord_and_user_override_remain_usable() {
    let (_root, mut controller) = controller();
    raw_key(
        &mut controller,
        KeyCode::Char('?'),
        KeyModifiers::SHIFT,
        crossterm::event::KeyEventKind::Press,
    );
    assert!(controller.app.help);
    let mut overrides = std::collections::BTreeMap::new();
    overrides.insert(
        "F12".into(),
        terminal_file_manager::input::Action::TogglePanes,
    );
    let bindings = Bindings::new(&overrides).unwrap();
    assert_eq!(
        bindings.action(KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE)),
        Some(terminal_file_manager::input::Action::TogglePanes)
    );
}

#[test]
fn footer_hit_testing_dispatches_the_labelled_action_and_ignores_disabled_tail() {
    let (_root, mut controller) = controller();
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 130, 24));
    let click = |column| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row: geometry.footer.y,
            modifiers: KeyModifiers::NONE,
        })
    };
    let narrow = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 80, 24));
    assert!(
        narrow
            .footer_controls
            .iter()
            .all(|control| control.area.right() <= 79)
    );
    controller.handle(click(79), &narrow, Duration::ZERO);
    assert!(!controller.app.sidebar);
    let info = geometry
        .footer_controls
        .iter()
        .find(|control| control.action == terminal_file_manager::input::Action::Metadata)
        .unwrap()
        .area
        .x;
    controller.handle(click(info), &geometry, Duration::ZERO);
    assert!(controller.app.popup);
    assert!(!controller.app.sidebar);
    key(&mut controller, "Esc");
    let quit = geometry
        .footer_controls
        .iter()
        .find(|control| control.action == terminal_file_manager::input::Action::Quit)
        .unwrap()
        .area
        .x;
    controller.handle(click(quit), &geometry, Duration::ZERO);
    assert!(controller.quit);
}

fn switch_filter_focus(controller: &mut Controller, mouse: bool) {
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    if mouse {
        let area = geometry.panes[1 - controller.app.active].unwrap();
        controller.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: area.x + 1,
                row: area.y,
                modifiers: KeyModifiers::NONE,
            }),
            &geometry,
            Duration::ZERO,
        );
    } else {
        controller.dispatch(terminal_file_manager::input::Action::FocusPane, &geometry);
    }
}

fn edit_across_focus(origin: usize, mouse: bool, finish: &str) {
    let (_root, mut c) = controller();
    c.app.dual = true;
    c.app.active = origin;
    let other = 1 - origin;
    c.app.panes[origin].set_filter("al".into());
    c.app.panes[other].set_filter("bet".into());
    c.app.panes[origin].select_all();
    c.app.panes[other].select_all();
    let cursor = c.app.panes[other].cursor.clone();
    let marks = c.app.panes[other].marks.clone();
    key(&mut c, "/");
    key(&mut c, "x");
    switch_filter_focus(&mut c, mouse);
    assert_eq!(c.app.active, other);
    key(&mut c, "Backspace");
    let geometry = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    c.handle(Event::Paste("p\n".into()), &geometry, Duration::ZERO);
    assert_eq!(c.app.panes[origin].filter, "alp");
    key(&mut c, finish);
    assert_eq!(c.app.active, other);
    assert_eq!(
        c.app.panes[origin].filter,
        if finish == "Esc" { "al" } else { "alp" }
    );
    assert_eq!(c.app.panes[origin].current().unwrap().name, "alpha");
    assert_eq!(c.app.panes[origin].marks.len(), 1);
    assert_eq!(c.app.panes[other].filter, "bet");
    assert_eq!(c.app.panes[other].cursor, cursor);
    assert_eq!(c.app.panes[other].marks, marks);
    switch_filter_focus(&mut c, mouse);
    assert_eq!(c.app.active, origin);
    assert_eq!(c.app.panes[origin].current().unwrap().name, "alpha");
}

#[test]
fn filter_edits_accept_and_cancel_in_the_origin_after_mouse_focus_changes() {
    for origin in 0..2 {
        edit_across_focus(origin, true, "Esc");
        edit_across_focus(origin, true, "Enter");
    }
}

#[test]
fn filter_edits_accept_and_cancel_in_the_origin_after_focus_actions() {
    for origin in 0..2 {
        edit_across_focus(origin, false, "Esc");
        edit_across_focus(origin, false, "Enter");
    }
}

#[test]
fn filter_editor_keeps_tab_keys_modal_until_accept_or_cancel() {
    let (_root, mut c) = controller();
    c.app.dual = true;
    key(&mut c, "/");
    for chord in ["Tab", "Shift+Tab"] {
        key(&mut c, chord);
        assert_eq!(c.app.active, 0);
        assert_eq!(c.app.panes[0].filter, "");
    }
    key(&mut c, "Esc");
    key(&mut c, "Tab");
    assert_eq!(c.app.active, 1);
}

fn raw_key(
    c: &mut Controller,
    code: KeyCode,
    modifiers: KeyModifiers,
    kind: crossterm::event::KeyEventKind,
) {
    let g = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    c.handle(
        Event::Key(KeyEvent::new_with_kind(code, modifiers, kind)),
        &g,
        Duration::ZERO,
    );
}

#[test]
fn emergency_exit_requires_exact_control_c_and_ignores_releases() {
    use crossterm::event::KeyEventKind::{Press, Release, Repeat};
    for modifiers in [
        KeyModifiers::ALT,
        KeyModifiers::SHIFT,
        KeyModifiers::SUPER,
        KeyModifiers::META,
    ] {
        let (_root, mut c) = controller();
        raw_key(
            &mut c,
            KeyCode::Char('c'),
            KeyModifiers::CONTROL | modifiers,
            Press,
        );
        assert!(!c.quit, "{modifiers:?}");
    }
    let (_root, mut c) = controller();
    raw_key(&mut c, KeyCode::Char('c'), KeyModifiers::CONTROL, Release);
    assert!(!c.quit);
    for kind in [Press, Repeat] {
        raw_key(&mut c, KeyCode::Char('c'), KeyModifiers::CONTROL, kind);
        assert!(c.quit);
        c.quit = false;
    }
}

#[test]
fn exact_emergency_exit_survives_all_modals_and_user_overrides() {
    use terminal_file_manager::input::Action;
    for modal in ["/", "Alt+v", "V", "?", "Alt+Enter", "F12"] {
        let (_root, mut c) = controller();
        c = Controller::new(
            c.app,
            Bindings::new(&std::collections::BTreeMap::from([
                ("Ctrl+c".into(), Action::Hidden),
                ("Ctrl+Alt+c".into(), Action::Diagnostics),
            ]))
            .unwrap(),
            Default::default(),
        );
        key(&mut c, modal);
        key(&mut c, "Ctrl+c");
        assert!(c.quit, "{modal}");
        assert!(!c.app.panes[0].show_hidden);
    }
    let (_root, mut c) = controller();
    c = Controller::new(
        c.app,
        Bindings::new(&std::collections::BTreeMap::from([(
            "Ctrl+Alt+c".into(),
            Action::Diagnostics,
        )]))
        .unwrap(),
        Default::default(),
    );
    key(&mut c, "Ctrl+Alt+c");
    assert!(!c.quit && c.app.diagnostics);
}
