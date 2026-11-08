//! Inactive borders keep a neutral accent without altering pane contents.
use colored_text::ColorLevel;
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
    fs,
    input::{Action, Bindings, Controller},
    settings::Settings,
    ui,
};

const LEVELS: [ColorLevel; 4] = [
    ColorLevel::NoColor,
    ColorLevel::Ansi16,
    ColorLevel::Ansi256,
    ColorLevel::TrueColor,
];

fn controller(path: &std::path::Path) -> Controller {
    let mut app = App::new(path.into());
    app.dual = true;
    for pane in 0..2 {
        app.complete_listing(
            pane,
            1,
            Ok(fs::list(path, &fs::IgnoreContext::isolated(), &false.into()).unwrap()),
        );
        app.panes[pane].cursor = None;
    }
    Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    )
}

fn draw(app: &App, theme: &Theme) -> (Buffer, ui::Geometry) {
    let mut terminal = Terminal::new(TestBackend::new(160, 16)).unwrap();
    let mut geometry = ui::geometry(app, Rect::new(0, 0, 160, 16));
    terminal
        .draw(|frame| geometry = ui::render(frame, app, theme))
        .unwrap();
    (terminal.backend().buffer().clone(), geometry)
}

#[test]
fn inactive_borders_are_neutral_and_follow_keyboard_focus_without_dimming_contents() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("plain"), "owned").unwrap();
    let mut c = controller(root.path());
    for level in LEVELS {
        let theme = Theme::new(level);
        for _ in 0..3 {
            let (buffer, geometry) = draw(&c.app, &theme);
            for pane in 0..2 {
                let area = geometry.panes[pane].unwrap();
                let active = pane == c.app.active;
                let border = &buffer[(area.x, area.y + 2)];
                let expected = if active {
                    theme.style(Role::Header).fg
                } else {
                    theme.resolve("#767676").unwrap()
                };
                assert_eq!(
                    border.fg,
                    expected.unwrap_or(Color::Reset),
                    "{level:?} pane {pane}"
                );
                assert_eq!(border.modifier.contains(Modifier::BOLD), active);
                assert!(!border.modifier.contains(Modifier::DIM));
                let name = &buffer[(area.x + 8, area.y + 3)];
                assert_eq!(name.symbol(), "p");
                assert!(!name.modifier.contains(Modifier::DIM));
                assert_eq!(
                    buffer[(area.x + 1, area.y)].symbol(),
                    if active { ">" } else { " " }
                );
            }
            assert!(c.dispatch(Action::FocusPane, &geometry).is_empty());
        }
    }
}

#[test]
fn border_configuration_is_strict_round_trips_and_resolves_at_each_capability() {
    assert!(
        Settings::parse("inactive_pane_border = 'invalid'")
            .unwrap_err()
            .contains("inactive_pane_border")
    );
    assert!(Settings::default().inactive_pane_border.is_none());
    let settings = Settings::parse("inactive_pane_border = '#a0a0a0'").unwrap();
    let saved = toml::to_string(&settings).unwrap();
    assert_eq!(
        Settings::parse(&saved).unwrap().inactive_pane_border,
        settings.inactive_pane_border
    );
    for level in LEVELS {
        let mut theme = Theme::new(level);
        let default = theme.style(Role::InactivePaneBorder);
        assert_eq!(default.fg, theme.resolve("#767676").unwrap());
        theme
            .set_inactive_pane_border(settings.inactive_pane_border.as_deref())
            .unwrap();
        let style = theme.style(Role::InactivePaneBorder);
        assert_eq!(style.fg, theme.resolve("#a0a0a0").unwrap());
        assert_eq!(style.bg, None);
        assert!(style.add_modifier.is_empty());
        assert!(theme.set_inactive_pane_border(Some("invalid")).is_err());
        theme.set_inactive_pane_border(None).unwrap();
        assert_eq!(theme.style(Role::InactivePaneBorder), default);
    }
}

#[test]
fn configured_border_changes_only_the_inactive_perimeter_after_focus_switching() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("plain"), "owned").unwrap();
    let mut c = controller(root.path());
    for level in LEVELS {
        let normal = Theme::new(level);
        let mut configured = Theme::new(level);
        configured
            .set_inactive_pane_border(Some("#eeeeee"))
            .unwrap();
        for _ in 0..3 {
            let (before, geometry) = draw(&c.app, &normal);
            let (after, _) = draw(&c.app, &configured);
            let area = geometry.panes[1 - c.app.active].unwrap();
            let mut changed = 0;
            for y in 0..16 {
                for x in 0..160 {
                    if before[(x, y)] != after[(x, y)] {
                        assert!(area.contains((x, y).into()));
                        assert!(
                            x == area.x
                                || x == area.right() - 1
                                || y == area.y
                                || y == area.bottom() - 1
                        );
                        changed += 1;
                    }
                }
            }
            assert_eq!(changed > 0, level != ColorLevel::NoColor);
            let border = &after[(area.x, area.y + 2)];
            assert_eq!(
                border.fg,
                configured
                    .style(Role::InactivePaneBorder)
                    .fg
                    .unwrap_or(Color::Reset)
            );
            assert!(!border.modifier.contains(Modifier::DIM));
            assert!(c.dispatch(Action::FocusPane, &geometry).is_empty());
        }
    }
}
