//! View dialogs share measured title controls and retain their own input state.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect, style::Modifier};
use std::time::Duration;
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Bindings, Controller, HelpTopic, chord},
    ui,
};

fn controller() -> Controller {
    Controller::new(
        App::new("owned-view-help".into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}
fn event(c: &mut Controller, event: Event, bounds: Rect) {
    let g = ui::geometry(&c.app, bounds);
    assert!(c.handle(event, &g, Duration::ZERO).is_empty());
}
fn key(c: &mut Controller, name: &str) {
    key_at(c, name, Rect::new(0, 0, 100, 32));
}
fn key_at(c: &mut Controller, name: &str, bounds: Rect) {
    let (code, modifiers) = chord(name).unwrap();
    event(c, Event::Key(KeyEvent::new(code, modifiers)), bounds);
}
fn click(c: &mut Controller, area: Rect, bounds: Rect) {
    event(
        c,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }),
        bounds,
    );
}
fn draw(c: &Controller, bounds: Rect, level: ColorLevel) -> (Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    let mut g = None;
    terminal
        .draw(|f| g = Some(ui::render(f, &c.app, &Theme::new(level))))
        .unwrap();
    (terminal.backend().buffer().clone(), g.unwrap())
}
fn editor() -> Controller {
    let mut c = controller();
    key(&mut c, "Alt+v");
    key(&mut c, "End");
    key(&mut c, "Enter");
    c
}
fn text(buffer: &Buffer) -> String {
    buffer.content().iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn both_help_keys_preserve_view_selection_settings_and_geometry() {
    for (open, topic) in [
        ("V", HelpTopic::ViewOptions),
        ("Alt+v", HelpTopic::ViewProfiles),
    ] {
        let mut c = controller();
        key(&mut c, open);
        key(&mut c, "Down");
        let bounds = Rect::new(0, 0, 100, 32);
        let original = ui::geometry(&c.app, bounds).menu_layout.unwrap();
        let selection = (
            c.app.view_menu,
            c.app.profile_menu,
            c.app.profile_choice,
            c.app.active,
        );
        let preferences = toml::to_string(&c.preferences).unwrap();
        for help in ["?", "F1"] {
            key(&mut c, help);
            assert_eq!(c.app.context_help.as_ref().unwrap().topic, topic);
            for blocked in ["Enter", "Space", "Down", "V", "Alt+v"] {
                key(&mut c, blocked);
            }
            event(&mut c, Event::Paste("ignored".into()), bounds);
            key(&mut c, "Esc");
            assert_eq!(
                selection,
                (
                    c.app.view_menu,
                    c.app.profile_menu,
                    c.app.profile_choice,
                    c.app.active
                )
            );
            let restored = ui::geometry(&c.app, bounds).menu_layout.unwrap();
            assert_eq!(original.outer, restored.outer);
            assert_eq!(original.body, restored.body);
            assert_eq!(toml::to_string(&c.preferences).unwrap(), preferences);
            assert!(!c.preferences_changed);
        }
        key(&mut c, "Esc");
        assert!(c.app.view_menu.is_none() && c.app.profile_menu.is_none());
    }
}

#[test]
fn save_help_keeps_question_literal_and_close_cancels_one_layer_at_a_time() {
    let mut c = editor();
    let original = toml::to_string(&c.preferences).unwrap();
    for character in ["界", "?", "é"] {
        key(&mut c, character);
    }
    assert_eq!(c.app.profile_name.as_deref(), Some("界?é"));
    let bounds = Rect::new(0, 0, 100, 32);
    let parent = ui::geometry(&c.app, bounds).menu_layout.unwrap();
    key(&mut c, "F1");
    assert_eq!(
        c.app.context_help.as_ref().unwrap().topic,
        HelpTopic::SaveProfile
    );
    key(&mut c, "Enter");
    event(&mut c, Event::Paste("other".into()), bounds);
    let g = ui::geometry(&c.app, bounds);
    click(&mut c, g.modal_close.unwrap(), bounds);
    assert!(c.app.context_help.is_none());
    assert_eq!(c.app.profile_name.as_deref(), Some("界?é"));
    assert_eq!(ui::geometry(&c.app, bounds).menu.unwrap(), parent.outer);
    click(&mut c, parent.close.unwrap(), bounds);
    assert!(c.app.profile_name.is_none() && c.app.profile_menu.is_some());
    let g = ui::geometry(&c.app, bounds);
    click(&mut c, g.modal_close.unwrap(), bounds);
    assert!(c.app.profile_menu.is_none());
    assert_eq!(toml::to_string(&c.preferences).unwrap(), original);
    assert!(!c.preferences_changed);
}

#[test]
fn title_buttons_use_shared_semantic_cells_at_all_colour_levels() {
    let bounds = Rect::new(0, 0, 100, 32);
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        for c in vec![
            controller_with_menu("V"),
            controller_with_menu("Alt+v"),
            editor(),
        ]
        .into_boxed_slice()
        {
            let (buffer, g) = draw(&c, bounds, level);
            let shell = g.menu_layout.unwrap();
            assert_eq!(g.context_control, shell.help);
            assert_eq!(g.modal_close, shell.close);
            let help = shell.help.unwrap();
            assert_eq!(buffer[(help.right(), help.y)].symbol(), "─");
            for (area, label) in [(help, "[?]"), (shell.close.unwrap(), "[×]")] {
                for (x, symbol) in (area.x..area.right()).zip(label.chars()) {
                    let cell = &buffer[(x, area.y)];
                    assert_eq!(cell.symbol(), symbol.to_string());
                    assert_eq!(
                        cell.fg,
                        Theme::new(level)
                            .style(Role::Key)
                            .fg
                            .unwrap_or(ratatui::style::Color::Reset)
                    );
                    assert!(!cell.modifier.contains(Modifier::UNDERLINED));
                }
            }
        }
    }
}
fn controller_with_menu(name: &str) -> Controller {
    let mut c = controller();
    key(&mut c, name);
    c
}

#[test]
fn common_terminals_show_every_view_option_and_fifteen_profile_rows() {
    for open in ["V", "Alt+v"] {
        let mut c = controller_with_menu(open);
        if open == "Alt+v" {
            c.app.profile_names = (0..13).map(|i| format!("Profile {i:02}")).collect();
        }
        let bounds = Rect::new(0, 0, 80, 24);
        let (buffer, g) = draw(&c, bounds, ColorLevel::NoColor);
        assert!(g.menu_layout.unwrap().body.height >= 15);
        let shown = text(&buffer);
        if open == "V" {
            assert!(shown.contains("Human sizes"));
            assert!(shown.contains("Preferred metadata"));
            assert!(shown.contains("Owner/group display"));
        } else {
            assert!(shown.contains("Profile 00"));
            assert!(shown.contains("Profile 12"));
            assert!(shown.contains("Save active as…"));
        }
    }
}

#[test]
fn short_menus_use_available_height_and_keep_last_selection_clickable() {
    for open in ["V", "Alt+v"] {
        let mut c = controller_with_menu(open);
        c.app.profile_names = (0..20).map(|i| format!("Profile {i:02}")).collect();
        let bounds = Rect::new(0, 0, 80, 10);
        key_at(&mut c, "End", bounds);
        if open == "V" {
            for _ in 0..15 {
                key_at(&mut c, "Down", bounds);
            }
        }
        let (buffer, g) = draw(&c, bounds, ColorLevel::NoColor);
        let shell = g.menu_layout.unwrap();
        assert_eq!(shell.outer.height, 8);
        assert!(text(&buffer).contains(if open == "V" {
            "Owner/group display"
        } else {
            "Save active as…"
        }));
        let row = Rect::new(shell.body.x, shell.body.bottom() - 1, 1, 1);
        click(&mut c, row, bounds);
        if open == "V" {
            assert!(c.preferences_changed);
        } else {
            assert_eq!(c.app.profile_name.as_deref(), Some(""));
        }
    }
}

#[test]
fn mouse_help_and_close_keep_view_changes_and_do_not_change_another_row() {
    let mut c = controller_with_menu("V");
    let bounds = Rect::new(0, 0, 80, 24);
    let g = ui::geometry(&c.app, bounds);
    let shell = g.menu_layout.unwrap();
    click(&mut c, shell.body, bounds);
    assert!(!c.app.panes[0].human_sizes);
    let changed = toml::to_string(&c.preferences).unwrap();
    click(&mut c, shell.help.unwrap(), bounds);
    assert_eq!(
        c.app.context_help.as_ref().unwrap().topic,
        HelpTopic::ViewOptions
    );
    click(&mut c, shell.body, bounds);
    assert_eq!(toml::to_string(&c.preferences).unwrap(), changed);
    key(&mut c, "Esc");
    click(&mut c, shell.close.unwrap(), bounds);
    assert!(c.app.view_menu.is_none());
    assert_eq!(toml::to_string(&c.preferences).unwrap(), changed);
}

#[test]
fn narrow_resize_keeps_f1_access_and_never_hits_padding_or_hints() {
    for mut c in vec![
        controller_with_menu("V"),
        controller_with_menu("Alt+v"),
        editor(),
    ]
    .into_boxed_slice()
    {
        for (width, height) in [
            (120, 32),
            (40, 14),
            (20, 7),
            (20, 8),
            (20, 9),
            (20, 10),
            (3, 3),
            (1, 1),
            (80, 24),
        ] {
            check_narrow_parent(&mut c, Rect::new(0, 0, width, height));
        }
    }
}

fn check_narrow_parent(c: &mut Controller, bounds: Rect) {
    let (width, height) = (bounds.width, bounds.height);
    event(c, Event::Resize(width, height), bounds);
    let (buffer, g) = draw(c, bounds, ColorLevel::NoColor);
    let shell = g.menu_layout.unwrap();
    assert_eq!(shell.outer.intersection(bounds), shell.outer);
    if width >= 20 && height >= 7 {
        assert!(
            shell.body.height > 0,
            "{width}x{height}: {shell:?}; editing={:?}",
            c.app.profile_name
        );
    }
    let before = (
        c.app.view_menu,
        c.app.profile_menu,
        c.app.profile_name.clone(),
    );
    click(c, shell.footer, bounds);
    assert_eq!(
        before,
        (
            c.app.view_menu,
            c.app.profile_menu,
            c.app.profile_name.clone()
        )
    );
    key_at(c, "F1", bounds);
    assert!(c.app.context_help.is_some());
    event(c, Event::Resize(width, height), bounds);
    draw(c, bounds, ColorLevel::NoColor);
    key_at(c, "Esc", bounds);
    assert_eq!(
        before,
        (
            c.app.view_menu,
            c.app.profile_menu,
            c.app.profile_name.clone()
        )
    );
    assert!(!text(&buffer).contains("Some("));
}

#[test]
fn configured_opening_hints_and_rebound_escape_keep_safe_title_controls() {
    use terminal_file_manager::input::Action;
    let bindings = Bindings::new(&std::collections::BTreeMap::from([
        ("F2".into(), Action::ViewOptions),
        ("F3".into(), Action::ViewProfiles),
        ("Esc".into(), Action::Delete),
    ]))
    .unwrap();
    let mut c = Controller::new(
        App::new("owned-view-help".into()),
        bindings,
        Default::default(),
    );
    let bounds = Rect::new(0, 0, 100, 32);
    for name in ["F2", "F3"] {
        key(&mut c, name);
        let g = ui::geometry(&c.app, bounds);
        click(&mut c, g.context_control.unwrap(), bounds);
        let (buffer, _) = draw(&c, bounds, ColorLevel::NoColor);
        let action = if name == "F2" {
            Action::ViewOptions
        } else {
            Action::ViewProfiles
        };
        let keys = &c
            .app
            .help_rows
            .iter()
            .find(|entry| entry.action == action)
            .unwrap()
            .keys;
        assert!(keys.contains(name));
        assert!(text(&buffer).contains(&format!("Open from the file list: {keys}.")));
        key(&mut c, "Esc");
        let g = ui::geometry(&c.app, bounds);
        click(&mut c, g.modal_close.unwrap(), bounds);
        assert!(c.app.view_menu.is_none() && c.app.profile_menu.is_none());
        assert!(c.app.operations.outbox.is_empty() && c.app.operations.dialog.is_none());
    }
}
