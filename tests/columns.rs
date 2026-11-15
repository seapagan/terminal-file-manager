//! Responsive columns use cached display widths and exact spacing budgets.
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend};
use std::sync::atomic::AtomicBool;
use terminal_file_manager::{app::App, color::Theme, fs, ui, view::Columns};

#[path = "columns/attributes.rs"]
mod attributes;

fn symbolic_heading() -> &'static str {
    if cfg!(windows) { "Attr" } else { "Attributes" }
}

fn symbolic_width() -> u16 {
    if cfg!(windows) { 4 } else { 10 }
}

fn loaded() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("界e\u{301}"), "one").unwrap();
    let mut app = App::new(root.path().into());
    for id in 0..2 {
        app.complete_listing(
            id,
            1,
            Ok(fs::list(
                root.path(),
                &fs::IgnoreContext::isolated(),
                &AtomicBool::new(false),
            )
            .unwrap()),
        );
    }
    (root, app)
}

fn row(app: &App, width: u16, y: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, 16)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    (0..width)
        .map(|x| terminal.backend().buffer()[(x, y)].symbol())
        .collect()
}

#[test]
fn attributes_fit_below_the_old_global_threshold_and_reflow_in_both_panes() {
    let (_root, mut app) = loaded();
    // The attribute value width joins the existing size/date/name/spacing budget.
    let boundary = 50 + symbolic_width();
    for (width, attrs) in [
        (boundary - 1, false),
        (boundary, true),
        (boundary + 1, true),
    ] {
        assert_eq!(row(&app, width, 1).contains(symbolic_heading()), attrs);
    }
    app.dual = true;
    for active in 0..2 {
        app.active = active;
        for percent in [40, 45, 50, 55, 60] {
            app.split_percent = percent;
            let header = row(&app, 140, 1);
            let expected = usize::from(140 * percent / 100 >= boundary)
                + usize::from(140 * (100 - percent) / 100 >= boundary);
            assert_eq!(
                header.matches(symbolic_heading()).count(),
                expected,
                "{percent}: {header}"
            );
        }
    }
}

#[test]
fn disabled_columns_release_width_and_cached_sizes_determine_boundaries() {
    let (_root, mut app) = loaded();
    app.panes[0].columns = Columns {
        size: false,
        modified: false,
        attributes: true,
        owner: false,
    };
    assert!(row(&app, 26 + symbolic_width(), 1).contains(symbolic_heading()));
    app.panes[0].columns = Columns::default();
    app.panes[0].entries[0].size = Some(1_234_567_890);
    app.panes[0].human_sizes = false;
    app.panes[0].refilter();
    assert_eq!(app.panes[0].measurements.number, 10);
    let boundary = 56 + symbolic_width();
    assert!(!row(&app, boundary - 1, 1).contains(symbolic_heading()));
    assert!(row(&app, boundary, 1).contains(symbolic_heading()));
    assert!(row(&app, boundary, 3).contains("1234567890"));
}
