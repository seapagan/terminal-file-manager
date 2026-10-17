//! Assert final cells, including gaps, metadata and unused pane space.
use colored_text::ColorLevel;
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};
use std::sync::atomic::AtomicBool;
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs::{self, IgnoreContext},
    ui,
};

fn fixture() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    for name in ["alpha", "beta"] {
        std::fs::write(root.path().join(name), "data").unwrap();
    }
    let mut app = App::new(root.path().into());
    for id in 0..2 {
        app.complete_listing(
            id,
            1,
            Ok(fs::list(
                root.path(),
                &IgnoreContext::isolated(),
                &AtomicBool::new(false),
            )
            .unwrap()),
        );
        for entry in &mut app.panes[id].entries {
            entry.ignored = Some(true);
        }
    }
    (root, app)
}

fn colours(level: ColorLevel, active: bool) -> (Color, Color) {
    match (level, active) {
        (ColorLevel::NoColor, _) => (Color::Reset, Color::Reset),
        (ColorLevel::Ansi16, true) => (Color::Black, Color::Cyan),
        (ColorLevel::Ansi16, false) => (Color::White, Color::Blue),
        (ColorLevel::Ansi256, true) => (Color::Indexed(15), Color::Indexed(24)),
        (ColorLevel::Ansi256, false) => (Color::Indexed(7), Color::Indexed(236)),
        (ColorLevel::TrueColor, true) => (Color::Rgb(245, 248, 250), Color::Rgb(35, 85, 125)),
        (ColorLevel::TrueColor, false) => (Color::Rgb(184, 192, 202), Color::Rgb(41, 47, 56)),
    }
}

#[test]
fn selection_covers_text_metadata_gaps_and_tail_after_focus_and_layout_changes() {
    let (_root, mut app) = fixture();
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        for (width, dual) in [
            (240, true),
            (80, true),
            (40, true),
            (7, false),
            (160, false),
        ] {
            app.dual = dual;
            for active in 0..2 {
                app.active = active;
                assert_panes(&app, level, width);
            }
        }
    }
}

fn assert_panes(app: &App, level: ColorLevel, width: u16) {
    let mut terminal = Terminal::new(TestBackend::new(width, 20)).unwrap();
    let mut geometry = ui::geometry(app, terminal.backend().buffer().area);
    terminal
        .draw(|frame| geometry = ui::render(frame, app, &Theme::new(level)))
        .unwrap();
    let buffer = terminal.backend().buffer();
    for (id, area) in geometry.panes.iter().enumerate() {
        let Some(area) = area else { continue };
        let active = id == app.active;
        let (fg, bg) = colours(level, active);
        let selected = app.panes[id]
            .visible
            .iter()
            .position(|index| {
                Some(&app.panes[id].entries[*index].path) == app.panes[id].cursor.as_ref()
            })
            .unwrap() as u16;
        if area.width >= 10 {
            for x in area.x + 1..area.x + 4 {
                assert_eq!(buffer[(x, area.y + 2 + selected)].symbol(), " ");
            }
        }
        for x in area.x + 1..area.right() - 1 {
            let cell = &buffer[(x, area.y + 2 + selected)];
            assert_eq!((cell.fg, cell.bg), (fg, bg), "{level:?} pane={id} x={x}");
            assert!(!cell.modifier.contains(Modifier::DIM));
            assert_eq!(
                cell.modifier.contains(Modifier::REVERSED),
                level == ColorLevel::NoColor
            );
            assert_eq!(cell.modifier.contains(Modifier::BOLD), active);
            let body = &buffer[(x, area.y + 5)];
            assert_eq!(body.bg, body_background(level, active));
        }
        assert_unselected(buffer, *area, level, active, selected);
    }
}

fn body_background(level: ColorLevel, active: bool) -> Color {
    match (level, active) {
        (ColorLevel::Ansi256, true) => Color::Indexed(234),
        (ColorLevel::TrueColor, true) => Color::Rgb(22, 28, 36),
        _ => Color::Reset,
    }
}

fn assert_unselected(
    buffer: &ratatui::buffer::Buffer,
    area: ratatui::layout::Rect,
    level: ColorLevel,
    active: bool,
    selected: u16,
) {
    let y = area.y + 2 + (1 - selected);
    let cells: Vec<_> = (area.x + 1..area.right() - 1)
        .map(|x| &buffer[(x, y)])
        .collect();
    assert!(
        cells
            .iter()
            .all(|cell| cell.bg == body_background(level, active))
    );
    assert!(
        cells
            .iter()
            .any(|cell| cell.modifier.contains(Modifier::DIM))
    );
    if area.width > 40 {
        assert!(
            cells
                .iter()
                .any(|cell| !cell.modifier.contains(Modifier::DIM) && cell.symbol() != " ")
        );
    }
}
