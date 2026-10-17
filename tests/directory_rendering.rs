//! Directory suffixes are display-only and remain adjacent after truncation.
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use std::{fs, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs::{IgnoreContext, list},
    ui,
    view::Columns,
};

fn loaded(root: &std::path::Path) -> App {
    let mut app = App::new(root.into());
    app.complete_listing(
        0,
        1,
        Ok(list(root, &IgnoreContext::isolated(), &AtomicBool::new(false)).unwrap()),
    );
    app.panes[0].columns = Columns {
        size: false,
        modified: false,
        attributes: false,
        owner: false,
    };
    app.panes[0].cursor = None;
    app
}

fn draw(app: &App, width: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

#[test]
fn directories_use_d_indicator_and_adjacent_suffix_without_changing_native_names() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("docs")).unwrap();
    let mut app = loaded(root.path());
    let buffer = draw(&app, 80);
    assert_eq!(buffer[(5, 2)].symbol(), "d");
    let row = (7..79).map(|x| buffer[(x, 2)].symbol()).collect::<String>();
    assert_eq!(row.trim_end(), "docs/");
    assert_eq!(app.panes[0].entries[0].name, "docs");
    assert_eq!(app.panes[0].entries[0].path, root.path().join("docs"));
    app.panes[0].set_filter("docs/".into());
    assert!(app.panes[0].visible.is_empty());
    app.panes[0].set_filter("docs".into());
    assert_eq!(app.panes[0].visible.len(), 1);
}

#[test]
fn unicode_directory_suffix_survives_narrow_name_columns() {
    let root = tempfile::tempdir().unwrap();
    let name = "界e\u{301}".repeat(30);
    fs::create_dir(root.path().join(&name)).unwrap();
    let app = loaded(root.path());
    for width in [3, 7, 8, 12, 20, 40, 80] {
        let buffer = draw(&app, width);
        let start = if width < 10 { 1 } else { 7 };
        let name = (start..width - 1)
            .map(|x| buffer[(x, 2)].symbol())
            .collect::<String>();
        assert!(name.trim_end().ends_with('/'), "{width}: {name}");
        assert!(!name.contains('\u{1b}'));
    }
}

#[cfg(unix)]
#[test]
fn enterable_directory_links_keep_link_indicator_without_directory_suffix() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("docs")).unwrap();
    std::os::unix::fs::symlink(root.path().join("docs"), root.path().join("alias")).unwrap();
    let app = loaded(root.path());
    let buffer = draw(&app, 80);
    let index = app.panes[0]
        .visible
        .iter()
        .position(|i| app.panes[0].entries[*i].name == "alias")
        .unwrap();
    let row = index as u16 + 2;
    assert_eq!(buffer[(5, row)].symbol(), "@");
    let name = (7..79)
        .map(|x| buffer[(x, row)].symbol())
        .collect::<String>();
    assert_eq!(name.trim_end(), "alias");
    assert!(
        app.panes[0]
            .entries
            .iter()
            .find(|e| e.name == "alias")
            .unwrap()
            .enterable
    );
}
