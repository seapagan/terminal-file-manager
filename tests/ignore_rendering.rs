//! Ignore classification and filename dimming remain independent of metadata styles.
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Modifier};
use std::{fs, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::App,
    color::Theme,
    fs::{IgnoreContext, list},
    ui,
};

fn ignored_app(root: &std::path::Path) -> App {
    fs::create_dir_all(root.join(".git/info")).unwrap();
    fs::write(root.join("global-ignore"), "AGENTS.md\n").unwrap();
    fs::write(root.join(".git/info/exclude"), "*.diff\ncodacy*.json\n").unwrap();
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    for name in ["AGENTS.md", "branch.diff", "codacy-pr-1.json", "local.log"] {
        fs::write(root.join(name), "data").unwrap();
    }
    let context = IgnoreContext {
        global_file: Some(root.join("global-ignore")),
        discover_global: false,
    };
    let mut app = App::new(root.into());
    app.complete_listing(
        0,
        1,
        Ok(list(root, &context, &AtomicBool::new(false)).unwrap()),
    );
    app.panes[0].set_filter("".into());
    app.panes[0].select_all();
    app.panes[0].cursor = None;
    app
}

fn draw(app: &App, level: ColorLevel) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(180, 16)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(level));
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

#[test]
fn global_and_local_ignores_dim_only_filename_cells() {
    let root = tempfile::tempdir().unwrap();
    let mut app = ignored_app(root.path());
    let targets: Vec<_> = app.panes[0]
        .visible
        .iter()
        .enumerate()
        .filter_map(|(row, index)| {
            let entry = &app.panes[0].entries[*index];
            (entry.ignored == Some(true))
                .then_some((row as u16 + 2, entry.name.to_str().unwrap().len() as u16))
        })
        .collect();
    assert_eq!(targets.len(), 4);
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        for entry in &mut app.panes[0].entries {
            entry.ignored = Some(false);
        }
        let plain = draw(&app, level);
        for entry in &mut app.panes[0].entries {
            entry.ignored = Some(true);
        }
        let dimmed = draw(&app, level);
        for (row, length) in &targets {
            for x in 1..179 {
                let mut expected = plain[(x, *row)].clone();
                if (8..8 + length).contains(&x) {
                    expected.modifier.insert(Modifier::DIM);
                }
                assert_eq!(dimmed[(x, *row)], expected, "column {x}, row {row}");
            }
        }
    }
}
