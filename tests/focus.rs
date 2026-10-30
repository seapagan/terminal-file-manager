//! Cursor styling wins over every coloured span without guessing terminal colours.
use colored_text::ColorLevel;
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, UNIX_EPOCH},
};
use terminal_file_manager::{
    app::App,
    color::{Role, Theme},
    fs::{IgnoreContext, list},
    ui,
};

fn marked_directory_app(root: &std::path::Path) -> App {
    std::fs::create_dir(root.join("directory")).unwrap();
    let mut app = App::new(root.into());
    app.dual = true;
    app.now = UNIX_EPOCH + Duration::from_secs(100_000);
    for id in 0..2 {
        app.complete_listing(
            id,
            1,
            Ok(list(root, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap()),
        );
        let pane = &mut app.panes[id];
        pane.entries[0].size = Some(2_000_000_000);
        pane.entries[0].modified = Some(app.now);
        pane.entries[0].mode = Some(0o040755);
        pane.entries[0].ignored = Some(true);
        pane.refilter();
        pane.select_all();
    }
    app
}

#[test]
fn cursors_override_directory_size_time_permission_and_mark_spans() {
    let root = tempfile::tempdir().unwrap();
    let mut app = marked_directory_app(root.path());
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        for active in 0..2 {
            app.active = active;
            let mut terminal = Terminal::new(TestBackend::new(240, 12)).unwrap();
            terminal
                .draw(|frame| {
                    ui::render(frame, &app, &Theme::new(level));
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            for id in 0..2 {
                let left = 1 + id * 120;
                assert_eq!(buffer[(left, 2)].symbol(), "[");
                assert_eq!(buffer[(left + 1, 2)].symbol(), "*");
                assert_eq!(buffer[(left + 2, 2)].symbol(), "]");
                for x in left..left + 118 {
                    assert_cursor(&buffer[(x, 2)], level, id as usize == active);
                }
            }
        }
    }
}

fn assert_cursor(cell: &ratatui::buffer::Cell, level: ColorLevel, active: bool) {
    let style = Theme::new(level).style(if active {
        Role::Cursor
    } else {
        Role::InactiveCursor
    });
    assert_eq!(
        (cell.fg, cell.bg),
        (
            style.fg.unwrap_or(Color::Reset),
            style.bg.unwrap_or(Color::Reset)
        )
    );
    assert_eq!(
        cell.modifier.contains(Modifier::REVERSED),
        level == ColorLevel::NoColor
    );
    assert_eq!(cell.modifier.contains(Modifier::BOLD), active);
    assert!(
        !cell
            .modifier
            .intersects(Modifier::DIM | Modifier::UNDERLINED)
    );
}
