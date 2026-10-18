//! Ranges own only newly added visible marks, keyed by native path.
use crossterm::event::{Event, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::{fs, sync::atomic::AtomicBool, time::Duration};
use terminal_file_manager::{
    app::App,
    fs::{IgnoreContext, list},
    input::{Action, Bindings, Controller, chord},
    ui,
};

fn loaded() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    for name in ["a", "b", "c", "d", "e"] {
        fs::write(root.path().join(name), "data").unwrap();
    }
    let snapshot = list(
        root.path(),
        &IgnoreContext::isolated(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let mut app = App::new(root.path().into());
    for id in 0..2 {
        app.complete_listing(id, 1, Ok(snapshot.clone()));
    }
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        ),
    )
}

fn key(c: &mut Controller, input: &str) {
    let (code, modifiers) = chord(input).unwrap();
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, 180, 12));
    c.handle(
        Event::Key(KeyEvent::new(code, modifiers)),
        &geometry,
        Duration::ZERO,
    );
}

fn marks(c: &Controller, pane: usize) -> Vec<String> {
    let mut names: Vec<_> = c.app.panes[pane]
        .marks
        .keys()
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    names.sort();
    names
}

fn owner_x(c: &Controller) -> u16 {
    use colored_text::ColorLevel;
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(180, 12)).unwrap();
    terminal
        .draw(|frame| {
            ui::render(
                frame,
                &c.app,
                &terminal_file_manager::color::Theme::new(ColorLevel::NoColor),
            );
        })
        .unwrap();
    let row = (0..180)
        .map(|x| terminal.backend().buffer()[(x, 1)].symbol())
        .collect::<String>();
    row.find("Owner").unwrap() as u16
}

#[test]
fn range_shrink_and_crossing_preserve_independent_marks_inside_and_outside() {
    let (_root, mut c) = loaded();
    for input in [
        "Space",
        "End",
        "Space",
        "Up",
        "Space",
        "Up",
        "Shift+Down",
        "Shift+Down",
    ] {
        key(&mut c, input);
    }
    assert_eq!(marks(&c, 0), ["a", "c", "d", "e"]);
    for input in ["Shift+Up", "Shift+Up", "Shift+Up"] {
        key(&mut c, input);
    }
    assert_eq!(marks(&c, 0), ["a", "b", "c", "d", "e"]);
    key(&mut c, "Shift+Down");
    assert_eq!(marks(&c, 0), ["a", "c", "d", "e"]);
    assert_eq!(c.app.panes[0].selection_counts().visible, 4);
    assert!(c.app.panes[1].marks.is_empty());
}

#[test]
fn panes_keep_independent_anchors_and_overlays_cannot_select_underlying_entries() {
    let (_root, mut c) = loaded();
    for input in [
        "Alt+p",
        "Shift+Down",
        "Tab",
        "Down",
        "Shift+Down",
        "Shift+Tab",
        "Shift+Down",
    ] {
        key(&mut c, input);
    }
    assert_eq!(marks(&c, 0), ["a", "b", "c"]);
    assert_eq!(marks(&c, 1), ["b", "c"]);
    for overlay in ["?", "Alt+Enter", "V", "Alt+v", "/"] {
        let before = c.app.panes[0].marks.clone();
        key(&mut c, overlay);
        key(&mut c, "Shift+Down");
        assert_eq!(c.app.panes[0].marks, before);
        key(&mut c, "Esc");
    }
}

#[test]
fn policy_changes_end_range_ownership_but_format_changes_keep_it() {
    for input in ["s", "v", "g", "h", "i"] {
        let (_root, mut c) = loaded();
        for _ in 0..3 {
            key(&mut c, "Shift+Down");
        }
        let before = c.app.panes[0].marks.clone();
        key(&mut c, input);
        key(&mut c, "Shift+Up");
        assert!(
            before.keys().all(|p| c.app.panes[0].marks.contains_key(p)),
            "{input}"
        );
    }
    let (_root, mut c) = loaded();
    for _ in 0..3 {
        key(&mut c, "Shift+Down");
    }
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, 180, 12));
    c.dispatch(Action::Permissions, &geometry);
    key(&mut c, "Shift+Up");
    assert_eq!(marks(&c, 0), ["a", "b", "c"]);
    c.app.panes[0].set_filter(String::new());
    key(&mut c, "Shift+Up");
    assert_eq!(marks(&c, 0), ["a", "b", "c"]);
}

#[test]
fn refresh_success_and_failure_reset_range_without_losing_existing_marks() {
    for success in [false, true] {
        let (root, mut c) = loaded();
        for _ in 0..3 {
            key(&mut c, "Shift+Down");
        }
        let request = c.app.request_listing(0, root.path().into());
        let result = if success {
            Ok(list(
                root.path(),
                &IgnoreContext::isolated(),
                &AtomicBool::new(false),
            )
            .unwrap())
        } else {
            Err("fixture denied".into())
        };
        c.app.complete_listing(0, request.generation, result);
        key(&mut c, "Shift+Up");
        assert_eq!(marks(&c, 0), ["a", "b", "c", "d"]);
    }
}

#[test]
fn shift_click_uses_visible_order_and_ignores_double_click_and_owner_toggle() {
    let (_root, mut c) = loaded();
    key(&mut c, "Down");
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, 180, 12));
    let column = owner_x(&c);
    let status = c.app.status.clone();
    for time in [0, 100] {
        c.handle(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row: 5,
                modifiers: KeyModifiers::SHIFT,
            }),
            &geometry,
            Duration::from_millis(time),
        );
        assert_eq!(marks(&c, 0), ["b", "c", "d"]);
        assert!(c.app.panes[0].pending.is_none());
        assert!(!c.preferences_changed);
        assert_eq!(c.app.status, status);
    }
    c.app.panes[0].set_filter("no matches".into());
    key(&mut c, "Shift+Down");
    assert!(c.app.panes[0].eligible().is_empty());
    assert_eq!(c.app.panes[0].selection_counts().excluded(), 3);
}

#[test]
fn scrolled_shift_click_targets_the_drawn_row_and_skips_missing_identities() {
    let (_root, mut c) = loaded();
    key(&mut c, "End");
    key(&mut c, "Shift+Up");
    let geometry = ui::geometry(&c.app, Rect::new(0, 0, 180, 8));
    assert!(geometry.listing_rows[0] > 0);
    let pane = &c.app.panes[0];
    let row = pane.viewport_start(geometry.listing_rows[0]);
    assert!(row > 0);
    let expected = pane.entries[pane.visible[row]].path.clone();
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 7,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        }),
        &geometry,
        Duration::ZERO,
    );
    assert_eq!(c.app.panes[0].cursor.as_ref(), Some(&expected));
    assert_eq!(marks(&c, 0), ["d", "e"]);
    key(&mut c, "Ctrl+u");
    key(&mut c, "Home");
    c.app.panes[0].entries[1].identity = None;
    key(&mut c, "Shift+Down");
    key(&mut c, "Shift+Down");
    assert_eq!(marks(&c, 0), ["a", "c"]);
}

#[test]
fn explicit_marks_commit_the_range_and_shift_bindings_remain_configurable() {
    let (_root, mut c) = loaded();
    for input in ["Shift+Down", "Shift+Down", "Space", "Shift+Up"] {
        key(&mut c, input);
    }
    assert_eq!(marks(&c, 0), ["a", "b", "c"]);
    let overrides = [("Shift+Down".into(), Action::Quit)].into();
    let bindings = Bindings::new(&overrides).unwrap();
    let event = |text| {
        let (code, mods) = chord(text).unwrap();
        KeyEvent::new(code, mods)
    };
    assert_eq!(bindings.action(event("Shift+Down")), Some(Action::Quit));
    assert_eq!(bindings.action(event("Shift+Tab")), Some(Action::FocusPane));
    assert_eq!(bindings.action(event("z")), Some(Action::Zoxide));
    assert_eq!(bindings.action(event("Z")), Some(Action::OppositeZoxide));
}

#[cfg(target_os = "linux")]
#[test]
fn range_marks_preserve_raw_native_names() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let (root, mut c) = loaded();
    let raw = root.path().join(OsString::from_vec(b"raw-\xff".to_vec()));
    fs::write(&raw, "native").unwrap();
    let request = c.app.request_listing(0, root.path().into());
    c.app.complete_listing(
        0,
        request.generation,
        Ok(list(
            root.path(),
            &IgnoreContext::isolated(),
            &AtomicBool::new(false),
        )
        .unwrap()),
    );
    c.app.panes[0].move_cursor(isize::MAX, 4);
    c.app.panes[0].extend_range(-1, 4);
    assert!(c.app.panes[0].marks.contains_key(&raw));
    assert_eq!(c.app.panes[0].selection_counts().visible, 2);
}
