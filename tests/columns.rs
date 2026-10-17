//! Responsive columns use cached display widths and exact spacing budgets.
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend};
use std::sync::atomic::AtomicBool;
use terminal_file_manager::{app::App, color::Theme, fs, ui, view::Columns};

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
    // Four size digits, one unit, 16 timestamp cells, 10 attribute cells,
    // 16 name cells, indicators and all six inter-column spaces: 58 cells.
    for (width, attrs) in [(59, false), (60, true), (61, true), (70, true)] {
        assert_eq!(row(&app, width, 1).contains("Mode/attrs"), attrs);
    }
    app.dual = true;
    for active in 0..2 {
        app.active = active;
        for percent in [40, 45, 50, 55, 60] {
            app.split_percent = percent;
            let header = row(&app, 140, 1);
            let expected = usize::from(percent >= 45) + usize::from(percent <= 55);
            assert_eq!(
                header.matches("Mode/attrs").count(),
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
    assert!(row(&app, 36, 1).contains("Mode/attrs"));
    app.panes[0].columns = Columns::default();
    app.panes[0].entries[0].size = Some(1_234_567_890);
    app.panes[0].human_sizes = false;
    app.panes[0].refilter();
    assert_eq!(app.panes[0].measurements.number, 10);
    assert!(!row(&app, 65, 1).contains("Mode/attrs"));
    assert!(row(&app, 66, 1).contains("Mode/attrs"));
    assert!(row(&app, 66, 2).contains("1234567890"));
    for width in 1..=66 {
        let _ = row(&app, width, 2);
    }
}
