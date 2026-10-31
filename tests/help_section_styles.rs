//! Section emphasis survives capability fallback, scrolling and landing expiry.
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
    input::{HelpSection, HelpTarget},
    ui,
};

const LEVELS: [ColorLevel; 4] = [
    ColorLevel::NoColor,
    ColorLevel::Ansi16,
    ColorLevel::Ansi256,
    ColorLevel::TrueColor,
];

fn app() -> App {
    let mut app = App::new("owned-section-style".into());
    app.help = true;
    app
}

fn assert_heading(buffer: &Buffer, title: &str, theme: &Theme) {
    let (x, y) = (0..buffer.area.height)
        .find_map(|y| {
            let row: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            row.find(title)
                .map(|x| (ratatui::text::Line::raw(&row[..x]).width() as u16, y))
        })
        .unwrap_or_else(|| panic!("missing {title}"));
    let foreground = theme.resolve("#89dceb").unwrap().unwrap_or(Color::Reset);
    let modifiers = if theme.level == ColorLevel::NoColor {
        Modifier::BOLD | Modifier::UNDERLINED
    } else {
        Modifier::BOLD
    };
    for offset in 0..title.len() as u16 {
        let cell = &buffer[(x + offset, y)];
        assert_eq!(cell.fg, foreground, "{title}");
        assert_eq!(cell.modifier, modifiers, "{title}");
    }
}

#[test]
fn every_section_glyph_uses_light_cyan_or_noncolour_emphasis() {
    let app = app();
    for level in LEVELS {
        let theme = Theme::new(level);
        let mut terminal = Terminal::new(TestBackend::new(120, 160)).unwrap();
        terminal
            .draw(|f| {
                ui::render(f, &app, &theme);
            })
            .unwrap();
        for section in HelpSection::ALL {
            assert_heading(terminal.backend().buffer(), section.title(), &theme);
        }
    }
}

#[test]
fn scrolling_each_section_into_eighty_columns_preserves_its_style() {
    let mut app = app();
    for level in LEVELS {
        let theme = Theme::new(level);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let layout = ui::geometry(&app, Rect::new(0, 0, 80, 24)).help.unwrap();
        for section in HelpSection::ALL {
            let target = HelpTarget::Section(section);
            let anchor = layout
                .anchors
                .iter()
                .find(|(candidate, _)| *candidate == target)
                .unwrap()
                .1;
            app.overlay_scroll = anchor.min(layout.scroll_limit);
            terminal
                .draw(|f| {
                    ui::render(f, &app, &theme);
                })
                .unwrap();
            assert_heading(terminal.backend().buffer(), section.title(), &theme);
        }
    }
}

#[test]
fn landing_expiry_restores_section_style_after_cursor_highlight() {
    use std::time::Duration;
    for level in LEVELS {
        let mut app = app();
        app.help_landing = Some((
            HelpTarget::Section(HelpSection::Navigation),
            app.now + Duration::from_secs(2),
        ));
        let theme = Theme::new(level);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let body = ui::geometry(&app, Rect::new(0, 0, 80, 24))
            .help
            .unwrap()
            .body;
        terminal
            .draw(|f| {
                ui::render(f, &app, &theme);
            })
            .unwrap();
        for offset in 0.."Navigation".len() as u16 {
            let cell = &terminal.backend().buffer()[(body.x + offset, body.y)];
            assert_eq!(cell.modifier, theme.style(Role::Cursor).add_modifier);
        }
        app.now += Duration::from_secs(3);
        terminal
            .draw(|f| {
                ui::render(f, &app, &theme);
            })
            .unwrap();
        assert_heading(terminal.backend().buffer(), "Navigation", &theme);
    }
}

#[test]
fn section_role_is_distinct_without_changing_shared_text_roles() {
    use ratatui::style::Style;
    for level in LEVELS {
        let theme = Theme::new(level);
        let section = theme.style(Role::SectionHeader);
        let key = theme.style(Role::Key);
        let expected_accent = match level {
            ColorLevel::NoColor => None,
            ColorLevel::Ansi16 => Some(Color::Cyan),
            ColorLevel::Ansi256 => Some(Color::Indexed(30)),
            ColorLevel::TrueColor => Some(Color::Rgb(35, 140, 150)),
        };
        assert_eq!(key.fg, expected_accent);
        assert_eq!(key.add_modifier, Modifier::BOLD);
        assert_eq!(theme.style(Role::Header), key);
        assert_eq!(theme.style(Role::Value), Style::default());
        assert_ne!(section, key);
        assert_eq!(section.fg, theme.resolve("#89dceb").unwrap());
        if level == ColorLevel::NoColor {
            assert!(section.add_modifier.contains(Modifier::UNDERLINED));
        } else {
            assert_ne!(section.fg, key.fg);
        }
    }
    assert_eq!(
        Theme::new(ColorLevel::TrueColor)
            .style(Role::SectionHeader)
            .fg,
        Some(Color::Rgb(0x89, 0xdc, 0xeb))
    );
}
