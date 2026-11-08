//! Geometry, rendered hover cells and modal bounds across pane layouts.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier},
};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    input::{Bindings, Controller},
    navigation::PathPicker,
    settings::Settings,
    ui,
};

fn draw(app: &App, theme: &Theme, width: u16, height: u16) -> (Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut g = ui::geometry(app, Rect::new(0, 0, width, height));
    terminal.draw(|f| g = ui::render(f, app, theme)).unwrap();
    (terminal.backend().buffer().clone(), g)
}

fn path() -> std::path::PathBuf {
    std::path::PathBuf::from(if cfg!(windows) {
        r"C:\tfm-fixture"
    } else {
        "/tfm-fixture"
    })
    .join("界e\u{301}")
    .join("long-parent".repeat(12))
    .join("leaf👩‍💻")
}

#[test]
fn header_hit_regions_match_rendered_cells_and_avoid_dividers_after_resize() {
    let mut app = App::new(path());
    app.panes[1].path = path().ancestors().last().unwrap().join("opposite");
    for dual in [false, true] {
        app.dual = dual;
        for maximized in [false, true] {
            app.maximized = maximized;
            for active in 0..2 {
                app.active = active;
                for width in [0, 1, 12, 16, 24, 40, 79, 80, 120, 240] {
                    let (buffer, g) = draw(&app, &Theme::new(ColorLevel::NoColor), width, 24);
                    for control in &g.path_controls {
                        let pane = g.panes[control.pane].unwrap();
                        assert!(control.area.x > pane.x);
                        assert!(control.area.right() < pane.right());
                        assert!(
                            g.divider
                                .is_none_or(|divider| !divider.intersects(control.area))
                        );
                        let rendered = (control.area.x..control.area.right())
                            .map(|x| buffer[(x, control.area.y)].symbol())
                            .collect::<String>();
                        assert_eq!(rendered.replace(' ', ""), control.text.replace(' ', ""));
                        if let Some(target) = &control.target {
                            assert!(control.source.ancestors().any(|p| p == target));
                        } else {
                            assert_eq!(control.text, "…");
                            assert_eq!(control.area.width, 1);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn both_panes_keep_cyan_breadcrumbs_and_configured_hover_after_focus_changes() {
    let mut app = App::new(path());
    app.dual = true;
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let mut theme = Theme::new(level);
        theme.set_path_hover(Some("#ffffff")).unwrap();
        for active in 0..2 {
            app.active = active;
            app.path_pointer = None;
            let (before, geometry) = draw(&app, &theme, 160, 24);
            for control in &geometry.path_controls {
                let old = &before[(control.area.x, control.area.y)];
                assert_eq!(old.fg, theme.style(Role::Header).fg.unwrap_or(Color::Reset));
                app.path_pointer = Some((control.area.x, control.area.y));
                let (after, _) = draw(&app, &theme, 160, 24);
                let new = &after[(control.area.x, control.area.y)];
                assert_eq!(
                    new.fg,
                    theme.style(Role::PathHover).fg.unwrap_or(Color::Reset)
                );
                assert_eq!(new.bg, old.bg);
                assert_eq!(new.underline_color, old.underline_color);
                if level == ColorLevel::NoColor {
                    assert!(!old.modifier.contains(Modifier::BOLD));
                    assert!(new.modifier.contains(Modifier::BOLD));
                } else {
                    assert_eq!(new.modifier, old.modifier);
                }
            }
        }
    }
}

#[test]
fn hover_changes_only_foreground_and_has_a_monochrome_fallback() {
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let mut app = App::new(path());
        let mut theme = Theme::new(level);
        theme.set_path_hover(Some("#ffffff")).unwrap();
        let (before, g) = draw(&app, &theme, 80, 24);
        for control in &g.path_controls {
            app.path_pointer = Some((control.area.x, control.area.y));
            let (after, _) = draw(&app, &theme, 80, 24);
            let line = ratatui::text::Line::raw(&control.text);
            let mut x = control.area.x;
            for grapheme in line.styled_graphemes(ratatui::style::Style::default()) {
                let old = &before[(x, control.area.y)];
                let new = &after[(x, control.area.y)];
                assert_eq!(new.bg, old.bg);
                assert_eq!(new.underline_color, old.underline_color);
                if level == ColorLevel::NoColor {
                    assert_eq!(new.fg, Color::Reset);
                    assert!(new.modifier.contains(Modifier::BOLD));
                    assert_ne!(new.modifier, old.modifier);
                } else {
                    assert_eq!(new.modifier, old.modifier);
                    assert_eq!(Some(new.fg), theme.style(Role::PathHover).fg);
                    assert_ne!(new.fg, old.fg);
                }
                x += ratatui::text::Line::raw(grapheme.symbol).width() as u16;
            }
        }
    }
}

#[test]
fn picker_wraps_complete_path_with_matching_mouse_targets_within_modal_bounds() {
    let mut app = App::new(path());
    app.path_picker = Some(PathPicker {
        pane: 0,
        path: path(),
        selected: 0,
        scroll: 0,
    });
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let theme = Theme::new(level);
        for width in [12, 24, 40, 80, 160] {
            let mut shown = String::new();
            let mut seen = std::collections::BTreeSet::new();
            for scroll in 0..100 {
                app.path_picker.as_mut().unwrap().scroll = scroll;
                let (_, g) = draw(&app, &theme, width, 24);
                let layout = g.path_picker.unwrap();
                let outer = layout.shell.outer;
                assert!(outer.width >= width.saturating_sub(2));
                assert_eq!(outer.intersection(Rect::new(1, 1, width - 2, 22)), outer);
                for (control, _) in &layout.controls {
                    assert_eq!(layout.shell.body.intersection(control.area), control.area);
                    if seen.insert((
                        layout.scroll + usize::from(control.area.y - layout.shell.body.y),
                        control.area.x,
                    )) {
                        shown.push_str(&control.text);
                    }
                    assert!(
                        control
                            .source
                            .ancestors()
                            .any(|p| Some(p) == control.target.as_deref())
                    );
                }
                if scroll >= layout.limit {
                    break;
                }
            }
            assert_eq!(shown, ui::clean(&path().to_string_lossy()));
        }
    }
}

#[test]
fn picker_normal_segments_match_the_header_breadcrumb_foreground() {
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let mut app = App::new(path());
        let theme = Theme::new(level);
        let (header, geometry) = draw(&app, &theme, 80, 24);
        let area = geometry.path_controls[0].area;
        let foreground = header[(area.x, area.y)].fg;
        assert_eq!(
            foreground,
            theme.style(Role::Header).fg.unwrap_or(Color::Reset)
        );
        app.path_picker = Some(PathPicker {
            pane: 0,
            path: path(),
            selected: 0,
            scroll: 0,
        });
        let (picker, geometry) = draw(&app, &theme, 80, 24);
        let layout = geometry.path_picker.unwrap();
        for (control, index) in &layout.controls {
            let cell = &picker[(control.area.x, control.area.y)];
            if *index == 0 {
                assert_eq!(
                    cell.fg,
                    theme.style(Role::PathHover).fg.unwrap_or(Color::Reset)
                );
                assert_eq!(cell.modifier, theme.style(Role::PathHover).add_modifier);
            } else {
                assert_eq!(cell.fg, foreground, "{level:?}: {}", control.text);
                assert!(cell.modifier.is_empty());
                if level != ColorLevel::NoColor {
                    assert_ne!(Some(cell.fg), theme.style(Role::PathHover).fg);
                }
            }
            assert_eq!(cell.bg, Color::Reset);
            assert_eq!(cell.underline_color, Color::Reset);
        }
    }
}

#[test]
fn configured_hover_is_strict_and_resolved_for_terminal_capabilities() {
    assert!(
        Settings::parse("path_hover = 'invalid-colour'")
            .unwrap_err()
            .contains("path_hover")
    );
    let settings = Settings::parse("path_hover = '#fafafa'").unwrap();
    assert_eq!(settings.path_hover.as_deref(), Some("#fafafa"));
    let saved = toml::to_string(&settings).unwrap();
    assert_eq!(
        Settings::parse(&saved).unwrap().path_hover,
        settings.path_hover
    );
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let mut theme = Theme::new(level);
        theme
            .set_path_hover(settings.path_hover.as_deref())
            .unwrap();
        assert_eq!(
            theme.style(Role::PathHover).fg,
            theme.resolve("#fafafa").unwrap()
        );
        assert_eq!(theme.style(Role::PathHover).bg, None);
        assert!(
            !theme
                .style(Role::PathHover)
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }
}

#[test]
fn keyboard_selection_reveals_offscreen_segments_and_tiny_picker_cannot_navigate() {
    let mut app = App::new(path());
    app.path_picker = Some(PathPicker {
        pane: 0,
        path: path(),
        selected: 0,
        scroll: 0,
    });
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    let g = ui::geometry(&c.app, Rect::new(0, 0, 24, 12));
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
        &g,
        Default::default(),
    );
    let g = ui::geometry(&c.app, Rect::new(0, 0, 24, 12));
    let state = c.app.path_picker.as_ref().unwrap();
    assert!(state.scroll > 0);
    assert!(
        g.path_picker
            .as_ref()
            .unwrap()
            .controls
            .iter()
            .any(|(_, i)| *i == state.selected)
    );
    assert_tiny_picker(&mut c, &g);
}

fn assert_tiny_picker(c: &mut Controller, g: &ui::Geometry) {
    c.handle(Event::Resize(1, 1), g, Default::default());
    let tiny = ui::geometry(&c.app, Rect::new(0, 0, 1, 1));
    assert!(
        c.handle(
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            &tiny,
            Default::default()
        )
        .is_empty()
    );
    assert!(c.app.panes[0].pending.is_none());
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }),
        &tiny,
        Default::default(),
    );
    c.handle(
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        &tiny,
        Default::default(),
    );
    assert!(c.app.path_picker.is_none());
}
