//! Count requests update the open Info surface without changing their pane owner.
use colored_text::ColorLevel;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::Duration;
use terminal_file_manager::{
    app::{App, Request, RequestKind},
    color::Theme,
    fs::{self, Totals},
    input::{Action, Bindings, Controller},
    ui,
};

fn loaded(sidebar: bool, popup: bool) -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(root.path().into());
    for id in 0..2 {
        let path = root.path().join(format!("directory{id}"));
        std::fs::create_dir(&path).unwrap();
        let entry = fs::inspect(&path);
        app.complete_listing(
            id,
            1,
            Ok(fs::Snapshot {
                directory: Some(Box::new(fs::inspect(root.path()))),
                path: root.path().into(),
                entries: vec![entry.clone()],
                errors: vec![],
            }),
        );
        let request = app.request_metadata(id).unwrap();
        app.complete_metadata(&request, entry, None);
        app.panes[id].sidebar = sidebar;
    }
    app.sidebar = sidebar;
    app.popup = popup;
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        ),
    )
}

fn count(c: &mut Controller, width: u16) -> Request {
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, width, 40));
    let requests = c.handle(
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
        &geometry,
        Duration::ZERO,
    );
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].kind, RequestKind::Totals);
    requests.into_iter().next().unwrap()
}

fn text(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(180, 40)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(frame, app, &Theme::new(ColorLevel::NoColor));
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

fn switch_pane(c: &mut Controller) {
    for action in [Action::TogglePanes, Action::FocusPane, Action::TogglePanes] {
        let geometry = ui::geometry(&c.app, Rect::new(0, 0, 180, 40));
        c.dispatch(action, &geometry);
    }
}

#[test]
fn count_keeps_the_existing_surface_and_renders_completed_or_failed_results() {
    for (sidebar, popup) in [(true, false), (false, true), (false, false), (true, true)] {
        for fails in [false, true] {
            let (_root, mut c) = loaded(sidebar, popup);
            let request = count(&mut c, 180);
            assert_eq!(c.app.popup, popup || !sidebar);
            assert_eq!(c.app.sidebar, sidebar);
            assert_eq!(request.pane, 0);
            assert!(text(&c.app).contains("Calculating totals"));
            let result = if fails {
                Err("count denied".into())
            } else {
                Ok(Totals {
                    files: 9,
                    ..Totals::default()
                })
            };
            assert!(c.app.complete_totals(&request, result));
            let rendered = text(&c.app);
            assert!(rendered.contains(if fails { "count denied" } else { "9 files" }));
            assert!(!rendered.contains("Calculating totals"));
            assert_eq!(c.app.popup, popup || !sidebar);
        }
    }
}

#[test]
fn count_opens_a_popup_when_a_requested_sidebar_is_collapsed() {
    let (_root, mut c) = loaded(true, false);
    count(&mut c, 80);
    assert!(c.app.popup);
}

#[test]
fn switching_panes_keeps_count_results_with_the_originating_pane_and_surface() {
    for (sidebar, popup) in [(true, false), (false, true)] {
        let (_root, mut c) = loaded(sidebar, popup);
        let request = count(&mut c, 180);
        assert_eq!(c.app.popup, popup);
        switch_pane(&mut c);
        assert_eq!(c.app.active, 1);
        assert!(c.app.details[0].counting);
        assert!(!c.app.details[1].counting);
        assert!(!text(&c.app).contains("Calculating totals"));
        assert!(c.app.complete_totals(
            &request,
            Ok(Totals {
                files: 9,
                ..Totals::default()
            })
        ));
        assert!(!text(&c.app).contains("9 files"));
        switch_pane(&mut c);
        assert!(text(&c.app).contains("9 files"));
        assert_eq!(c.app.popup, popup);
    }
}
