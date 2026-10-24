//! Pane headers retain directory identity inside their actual border budget.
use colored_text::ColorLevel;
use ratatui::{Terminal, backend::TestBackend};
use terminal_file_manager::{app::App, color::Theme, ui};

const LONG: &str = "/home/seapagan/data/work/own/terminal-file-manager";

fn titles(app: &App, width: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
    let mut areas = Vec::new();
    terminal
        .draw(|frame| {
            areas = ui::render(frame, app, &Theme::new(ColorLevel::NoColor))
                .panes
                .into_iter()
                .flatten()
                .collect();
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    areas
        .into_iter()
        .filter(|area| area.width >= 2)
        .map(|area| {
            assert_eq!(buffer[(area.x, area.y)].symbol(), "┌");
            assert_eq!(buffer[(area.right() - 1, area.y)].symbol(), "┐");
            (area.x + 1..area.right() - 1)
                .map(|x| buffer[(x, area.y)].symbol())
                .collect()
        })
        .collect()
}

#[test]
fn narrow_dual_headers_keep_the_complete_directory_tail() {
    let mut app = App::new(LONG.into());
    app.dual = true;
    for width in [80, 90, 100] {
        let headers = titles(&app, width);
        for header in &headers {
            assert!(
                header.contains("…/")
                    && header
                        .trim_end_matches('─')
                        .ends_with("/terminal-file-manager"),
                "{width}: {header}"
            );
        }
        assert!(headers[0].starts_with("> Pane 1: "));
        assert!(headers[1].starts_with("  Pane 2: "));
    }
    assert_eq!(app.panes[0].path.as_os_str(), LONG);
    assert_eq!(app.panes[1].path.as_os_str(), LONG);
}

#[test]
fn loading_suffix_and_directory_tail_share_the_measured_budget() {
    let mut app = App::new(LONG.into());
    let ready = titles(&app, 62).remove(0);
    assert_eq!(ready, format!("> Pane 1: {LONG}"));
    app.panes[0].pending = Some("/another/destination".into());
    let header = titles(&app, 50).remove(0);
    assert!(
        header.contains("…/")
            && header
                .trim_end_matches('─')
                .ends_with("/terminal-file-manager [loading]"),
        "{header}"
    );
    assert_eq!(
        app.panes[0].pending.as_deref(),
        Some(std::path::Path::new("/another/destination"))
    );
    app.panes[0].pending = None;
    assert_eq!(titles(&app, 62)[0], ready);
}

#[test]
fn path_budgets_preserve_exact_fits_and_elide_only_complete_middle_components() {
    let app = App::new("/alpha/beta/gamma/leaf".into());
    for (budget, expected) in [
        (22, "/alpha/beta/gamma/leaf"),
        (21, "/alpha/beta/…/leaf"),
        (18, "/alpha/beta/…/leaf"),
        (17, "/alpha/…/leaf"),
        (13, "/alpha/…/leaf"),
        (12, "/…/leaf"),
        (7, "/…/leaf"),
        (6, "…/leaf"),
        (5, "…leaf"),
        (4, "leaf"),
        (3, "…af"),
        (1, "…"),
        (0, ""),
    ] {
        assert_eq!(
            titles(&app, budget + 12)[0].trim_end_matches('─'),
            format!("> Pane 1: {expected}")
        );
    }
    let app = App::new("/a/verylongparent/b/leaf".into());
    assert_eq!(titles(&app, 23)[0], "> Pane 1: /a/…/b/leaf");
}

#[test]
fn balanced_middle_elision_uses_whole_components_at_both_ends() {
    for (path, budget, expected) in [
        (LONG, 24, "/…/terminal-file-manager"),
        (LONG, 29, "/home/…/terminal-file-manager"),
        (LONG, 38, "/home/seapagan/…/terminal-file-manager"),
        (LONG, 42, "/home/seapagan/…/own/terminal-file-manager"),
        ("/a/b/c/d/e/final", 14, "/a/b/…/e/final"),
        ("/long-leading-name/b/c/d/final", 14, "/…/b/c/d/final"),
        ("/a/b/c/long-trailing-name/final", 14, "/a/b/c/…/final"),
        ("/lengthy/b/c/d/final", 18, "/lengthy/b/…/final"),
        ("/界/e\u{301}/middle/d/leaf", 12, "/界 /e\u{301}/…/leaf"),
    ] {
        let app = App::new(path.into());
        assert_eq!(
            titles(&app, budget + 12)[0].trim_end_matches('─'),
            format!("> Pane 1: {expected}"),
            "{path}, budget {budget}"
        );
        assert_eq!(
            app.panes[0].path.as_os_str().as_encoded_bytes(),
            path.as_bytes()
        );
    }
}

#[test]
fn both_panes_retain_leading_context_at_their_independent_widths_after_resize() {
    let mut app = App::new(LONG.into());
    app.dual = true;
    assert_eq!(
        titles(&app, 100),
        [
            "> Pane 1: /home/seapagan/…/terminal-file-manager",
            "  Pane 2: /home/seapagan/…/terminal-file-manager",
        ]
    );
    assert_eq!(
        titles(&app, 82),
        [
            "> Pane 1: /home/…/terminal-file-manager",
            "  Pane 2: /home/…/terminal-file-manager",
        ]
    );
    app.split_percent = 41;
    assert_eq!(
        titles(&app, 100),
        [
            "> Pane 1: /home/…/terminal-file-manager",
            "  Pane 2: /home/seapagan/data/…/own/terminal-file-manager",
        ]
    );
    app.maximized = true;
    assert!(titles(&app, 100)[0].contains(LONG));
    app.maximized = false;
    assert_eq!(
        titles(&app, 100)[0],
        "> Pane 1: /home/…/terminal-file-manager"
    );
}

#[test]
fn short_root_relative_and_trailing_separator_paths_keep_their_spelling() {
    for path in [
        "/",
        "//",
        "",
        ".",
        "..",
        "leaf",
        "/leaf/",
        "alpha//beta/../leaf/",
    ] {
        let app = App::new(path.into());
        let width = ratatui::text::Line::raw(path).width() as u16 + 12;
        assert_eq!(titles(&app, width)[0], format!("> Pane 1: {path}"));
    }
    let app = App::new("/alpha/beta/gamma/leaf/".into());
    assert_eq!(
        titles(&app, 20)[0].trim_end_matches('─'),
        "> Pane 1: /…/leaf/"
    );
    let app = App::new("alpha/beta/gamma/leaf".into());
    assert_eq!(titles(&app, 18)[0], "> Pane 1: …/leaf");
}

#[test]
fn unicode_tail_fallback_keeps_wide_and_combining_graphemes_in_actual_cells() {
    assert_eq!(
        titles(&App::new("/界/e\u{301}".into()), 17)[0],
        "> Pane 1: /界 /e\u{301}"
    );
    let app = App::new(format!("/alpha/{}e\u{301}界👩‍💻", "x".repeat(1000)).into());
    // The extra blank cells are the trailing cells of the two wide graphemes.
    assert_eq!(titles(&app, 18)[0], "> Pane 1: …e\u{301}界 👩‍💻 ");
    assert_eq!(titles(&app, 15)[0], "> Pane 1: …👩‍💻 ");
    assert_eq!(titles(&app, 14)[0], "> Pane 1: …─");
    assert_eq!(titles(&app, 13)[0], "> Pane 1: …");
}

#[test]
fn control_escaping_precedes_width_measurement_and_elision() {
    let app = App::new("/alpha/long-parent/a\n\u{1b}\u{202e}".into());
    assert_eq!(
        titles(&app, 32)[0].trim_end_matches('─'),
        "> Pane 1: /…/a\\n\\u{1b}\\u{202e}"
    );
    for width in 0..60 {
        for header in titles(&app, width) {
            assert!(!header.chars().any(char::is_control));
            assert!(!header.contains('\u{202e}'));
        }
    }
    assert_eq!(
        app.panes[0].path.as_os_str(),
        "/alpha/long-parent/a\n\u{1b}\u{202e}"
    );
}

#[test]
fn resize_maximise_and_pane_toggles_recompute_titles_without_changing_navigation() {
    use ratatui::layout::Rect;
    use terminal_file_manager::{
        input::{Action, Bindings, Controller},
        settings::Settings,
    };
    let app = navigated_app();
    let paths = app
        .panes
        .each_ref()
        .map(|pane| pane.path.as_os_str().as_encoded_bytes().to_vec());
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 100, 12));
    let dual = titles(&controller.app, 100);
    assert!(dual[0].contains('…'));
    assert!(dual[1].contains("/different/pane/other"));
    controller.dispatch(Action::Maximise, &geometry);
    assert!(titles(&controller.app, 100)[0].contains(LONG));
    controller.dispatch(Action::Maximise, &geometry);
    assert_eq!(titles(&controller.app, 100), dual);
    controller.dispatch(Action::TogglePanes, &geometry);
    assert!(titles(&controller.app, 100)[0].contains(LONG));
    controller.dispatch(Action::TogglePanes, &geometry);
    assert_eq!(titles(&controller.app, 100), dual);
    for width in [79, 40, 24, 12, 11, 10, 3, 2, 1, 0, 80, 100] {
        titles(&controller.app, width);
    }
    assert_eq!(titles(&controller.app, 100), dual);
    assert!(controller.app.dual);
    assert_eq!(controller.app.active, 0);
    for (pane, original) in controller.app.panes.iter().zip(paths) {
        assert_eq!(pane.path.as_os_str().as_encoded_bytes(), original);
        assert!(pane.pending.is_none());
    }
    assert_eq!(
        controller.app.history(false).unwrap().path.as_os_str(),
        "/initial"
    );
}

fn navigated_app() -> App {
    let mut app = App::new("/initial".into());
    let request = app.request_listing(0, LONG.into());
    app.complete_listing(
        0,
        request.generation,
        Ok(terminal_file_manager::fs::Snapshot {
            directory: None,
            path: LONG.into(),
            entries: Vec::new(),
            errors: Vec::new(),
        }),
    );
    app.panes[1].path = "/different/pane/other".into();
    app.dual = true;
    app
}

#[test]
fn divider_drag_changes_each_title_budget_independently() {
    use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;
    use terminal_file_manager::{
        input::{Bindings, Controller},
        settings::Settings,
    };
    let mut app = App::new(LONG.into());
    app.dual = true;
    let mut controller = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    );
    let geometry = ui::geometry(&controller.app, Rect::new(0, 0, 160, 12));
    let before = titles(&controller.app, 160);
    assert!(before.iter().all(|header| header.contains(LONG)));
    for (kind, column) in [
        (
            MouseEventKind::Down(MouseButton::Left),
            geometry.divider.unwrap().x,
        ),
        (MouseEventKind::Drag(MouseButton::Left), 40),
        (MouseEventKind::Up(MouseButton::Left), 40),
    ] {
        controller.handle(
            Event::Mouse(MouseEvent {
                kind,
                column,
                row: 5,
                modifiers: KeyModifiers::NONE,
            }),
            &geometry,
            std::time::Duration::ZERO,
        );
    }
    assert_eq!(controller.app.split_percent, 25);
    let after = titles(&controller.app, 160);
    assert!(after[0].contains("…"));
    assert!(
        after[0]
            .trim_end_matches('─')
            .ends_with("/terminal-file-manager")
    );
    assert!(after[1].contains(LONG));
    let resized = titles(&controller.app, 80);
    assert!(resized.iter().all(|header| header.contains('…')));
    assert_eq!(titles(&controller.app, 160), after);
}

#[cfg(unix)]
#[test]
fn unix_non_utf8_and_backslash_names_keep_native_bytes() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let raw = b"/alpha/beta/dir\\name-\xff";
    let app = App::new(OsStr::from_bytes(raw).into());
    let header = titles(&app, 28).remove(0);
    assert!(header.contains("/…/dir\\name-�"), "{header}");
    assert_eq!(app.panes[0].path.as_os_str().as_bytes(), raw);
}

#[cfg(windows)]
#[test]
fn windows_drive_unc_and_extended_anchors_survive_middle_elision() {
    for (path, expected) in [
        (r"C:\alpha\beta\leaf", r"C:\…\leaf"),
        (r"C:/alpha/beta/leaf", "C:/…/leaf"),
        (r"C:alpha\beta\leaf", r"C:…\leaf"),
        (r"\\server\share\alpha\beta\leaf", r"\\server\share\…\leaf"),
        (r"\\?\C:\alpha\beta\leaf", r"\\?\C:\…\leaf"),
        (
            r"\\?\UNC\server\share\alpha\beta\leaf",
            r"\\?\UNC\server\share\…\leaf",
        ),
    ] {
        let app = App::new(path.into());
        assert_eq!(
            titles(&app, expected.len() as u16 + 10)[0],
            format!("> Pane 1: {expected}")
        );
        assert_eq!(
            app.panes[0].path.as_os_str().as_encoded_bytes(),
            path.as_bytes()
        );
    }
    for path in [
        r"C:\",
        r"\\server\share\",
        r"\\?\C:\",
        r"\\?\UNC\server\share\",
    ] {
        assert_eq!(
            titles(&App::new(path.into()), path.len() as u16 + 12)[0],
            format!("> Pane 1: {path}")
        );
    }
}
