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
fn files_and_mutations_explain_unavailability_without_launching() {
    let (_root, mut controller) = controller();
    key(&mut controller, "Enter");
    assert!(controller.app.status.contains("M6"));
    key(&mut controller, "F8");
    assert!(controller.app.status.contains("M2"));
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
    key(&mut controller, "End");
    for (width, height) in [(1, 1), (12, 5), (80, 10)] {
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
            assert!(text.contains(&controller.app.help_rows.last().unwrap().1));
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
    key(&mut controller, "Shift+?");
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
    let geometry = ui::geometry(&controller.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    let click = |column| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row: geometry.footer.y,
            modifiers: KeyModifiers::NONE,
        })
    };
    controller.handle(click(119), &geometry, Duration::ZERO);
    assert!(!controller.app.sidebar);
    let info = geometry
        .footer_controls
        .iter()
        .find(|control| control.action == terminal_file_manager::input::Action::ToggleSidebar)
        .unwrap()
        .area
        .x;
    controller.handle(click(info), &geometry, Duration::ZERO);
    assert!(controller.app.sidebar);
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
