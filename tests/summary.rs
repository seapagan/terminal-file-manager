//! Summary text distinguishes the visible directory from excluded marked entries.
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend};
use std::sync::atomic::AtomicBool;
use terminal_file_manager::{
    app::{App, IgnoreMode},
    color::Theme,
    fs, ui,
};

fn loaded() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    for name in ["visible", "hidden", "ignored", "filtered"] {
        std::fs::write(root.path().join(name), "data").unwrap();
    }
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

fn text(app: &App, width: u16, level: ColorLevel) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, 16)).unwrap();
    terminal
        .draw(|f| {
            ui::render(f, app, &Theme::new(level));
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .chunks(width.into())
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn no_selection_has_no_zero_breakdown_and_unmarked_hidden_entries_do_not_count() {
    let (_root, mut app) = loaded();
    app.panes[0]
        .entries
        .iter_mut()
        .find(|entry| entry.name == "hidden")
        .unwrap()
        .hidden = true;
    app.panes[0].refilter();
    let rendered = text(&app, 120, ColorLevel::NoColor);
    assert!(rendered.contains("3 items  │  No selection"));
    assert!(!rendered.contains("0 hidden"));
    app.panes[0].set_filter("no match".into());
    assert!(text(&app, 120, ColorLevel::NoColor).contains("0 items  │  No selection"));
}

#[test]
fn excluded_breakdowns_count_only_marks_with_existing_reason_precedence() {
    let (_root, mut app) = loaded();
    let pane = &mut app.panes[0];
    pane.select_all();
    for entry in &mut pane.entries {
        entry.hidden = entry.name == "hidden";
        entry.ignored = Some(entry.name == "hidden" || entry.name == "ignored");
    }
    pane.ignore = IgnoreMode::Hide;
    pane.set_filter("visible".into());
    for level in [
        ColorLevel::NoColor,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::TrueColor,
    ] {
        let rendered = text(&app, 160, level);
        assert!(rendered.contains(
            "1 items  │  Selected: 1 visible + 3 excluded (1 hidden · 1 ignored · 1 filtered)"
        ));
    }
    app.active = 1;
    assert!(text(&app, 120, ColorLevel::NoColor).contains("4 items  │  No selection"));
}

#[test]
fn narrow_summary_preserves_visible_and_excluded_selection_counts() {
    let (_root, mut app) = loaded();
    app.panes[0].select_all();
    app.panes[0].set_filter("visible".into());
    for width in [20, 28, 40, 60] {
        let rendered = text(&app, width, ColorLevel::NoColor);
        assert!(rendered.contains("1 items"));
        let expected = if width == 60 {
            "Selected: 1 visible + 3 excluded"
        } else {
            "1 sel +3 excl"
        };
        assert!(rendered.contains(expected), "{width}: {rendered}");
    }
}

#[test]
fn visible_only_and_excluded_only_summaries_omit_zero_reason_groups() {
    let (_root, mut app) = loaded();
    app.panes[0].select_all();
    let visible = text(&app, 120, ColorLevel::TrueColor);
    assert!(visible.contains("4 items  │  Selected: 4 visible"));
    assert!(!visible.contains("excluded"));
    app.panes[0].set_filter("no match".into());
    let excluded = text(&app, 120, ColorLevel::Ansi16);
    assert!(excluded.contains("0 items  │  Selected: 0 visible + 4 excluded (4 filtered)"));
    assert!(!excluded.contains("0 hidden"));
    assert!(!excluded.contains("0 ignored"));
}

fn many_entries() -> (tempfile::TempDir, App) {
    let (root, mut app) = loaded();
    for i in 0..12 {
        std::fs::write(root.path().join(format!("more{i}")), "data").unwrap();
    }
    let request = app.request_listing(0, root.path().into());
    app.complete_listing(
        0,
        request.generation,
        Ok(fs::list(
            root.path(),
            &fs::IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    app.panes[0].select_all();
    (root, app)
}

#[test]
fn narrow_summary_row_budget_matches_mouse_and_scroll_geometry() {
    use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;
    use terminal_file_manager::input::{Action, Bindings, Controller};
    let (_root, app) = many_entries();
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    let narrow = ui::geometry(&c.app, Rect::new(0, 0, 20, 16));
    let wide = ui::geometry(&c.app, Rect::new(0, 0, 120, 16));
    assert_eq!(narrow.listing_rows[0] + 1, wide.listing_rows[0]);
    let cursor = c.app.panes[0].cursor.clone();
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 8,
            row: 2 + narrow.listing_rows[0] as u16,
            modifiers: KeyModifiers::NONE,
        }),
        &narrow,
        std::time::Duration::ZERO,
    );
    assert_eq!(c.app.panes[0].cursor, cursor);
    c.dispatch(Action::End, &narrow);
    assert_eq!(c.app.panes[0].cursor_index(), Some(15));
    c.dispatch(Action::PageUp, &narrow);
    assert_eq!(
        c.app.panes[0].cursor_index(),
        Some(15 - narrow.listing_rows[0])
    );
}
