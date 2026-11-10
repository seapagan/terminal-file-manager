use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::{path::Path, sync::atomic::AtomicBool};
use terminal_file_manager::{
    app::App,
    fs,
    input::{Bindings, Controller},
    settings::Settings,
    ui,
};

fn controller(root: &Path) -> Controller {
    let mut app = App::new(root.into());
    app.launch_directory = Some(root.into());
    for pane in 0..2 {
        let snapshot = fs::list(root, &Default::default(), &AtomicBool::new(false)).unwrap();
        app.complete_listing(pane, 1, Ok(snapshot));
    }
    Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    )
}

fn send(c: &mut Controller, code: KeyCode) {
    let g = ui::geometry(&c.app, Rect::new(0, 0, 120, 24));
    assert!(
        c.handle(
            Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            &g,
            Default::default()
        )
        .is_empty()
    );
}

fn sequence(c: &mut Controller, keys: &str) {
    for ch in keys.chars() {
        send(c, KeyCode::Char(ch));
    }
}

#[test]
fn literal_highlighted_current_and_visible_marked_outputs_use_native_paths() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a λ;$(touch nope)"), "").unwrap();
    std::fs::create_dir(root.path().join("b界")).unwrap();
    let mut c = controller(root.path());
    c.app.panes[0].sort.grouping = terminal_file_manager::listing::Grouping::Mixed;
    c.app.panes[0].resort();
    c.app.panes[0].cursor = Some(root.path().join("a λ;$(touch nope)"));
    sequence(&mut c, "yhn");
    assert_eq!(
        c.path_copy_request.take().unwrap().text,
        "a λ;$(touch nope)"
    );
    sequence(&mut c, "ycrk");
    assert_eq!(c.path_copy_request.take().unwrap().text, ".");
    c.app.panes[0].select_all();
    sequence(&mut c, "ymn");
    assert_eq!(
        c.path_copy_request.take().unwrap().text,
        "a λ;$(touch nope)\nb界"
    );
}

#[test]
fn parent_defaults_current_marks_win_and_back_does_not_navigate() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), "").unwrap();
    let mut c = controller(root.path());
    c.app.panes[0].move_cursor(-1, 20);
    let before = c.app.panes[0].path.clone();
    sequence(&mut c, "yh");
    assert_eq!(c.app.footer_workflow.as_ref().unwrap().stage_index(), 0);
    send(&mut c, KeyCode::Enter);
    send(&mut c, KeyCode::Backspace);
    assert_eq!(
        c.app
            .footer_workflow
            .as_ref()
            .unwrap()
            .highlighted()
            .unwrap()
            .id,
        "current"
    );
    send(&mut c, KeyCode::Backspace);
    assert!(c.app.footer_workflow.is_none());
    assert_eq!(c.app.panes[0].path, before);
    assert!(c.path_copy_request.is_none());
    c.app.panes[0].select_all();
    sequence(&mut c, "y");
    assert_eq!(
        c.app
            .footer_workflow
            .as_ref()
            .unwrap()
            .highlighted()
            .unwrap()
            .id,
        "marked"
    );
    send(&mut c, KeyCode::Enter);
    send(&mut c, KeyCode::Enter);
    assert_eq!(
        c.path_copy_request.take().unwrap().text,
        root.path().join("file").to_str().unwrap()
    );
}

#[test]
fn snapshot_accepts_refresh_but_rejects_replaced_native_selection() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    std::fs::write(&file, "old").unwrap();
    let mut c = controller(root.path());
    sequence(&mut c, "yh");
    let request = c.app.request_refresh(0).unwrap();
    c.app.complete_listing(
        0,
        request.generation,
        Ok(fs::list(root.path(), &Default::default(), &AtomicBool::new(false)).unwrap()),
    );
    sequence(&mut c, "f");
    assert!(c.path_copy_request.take().is_some());
    sequence(&mut c, "yh");
    std::fs::rename(&file, root.path().join("old")).unwrap();
    std::fs::write(&file, "new").unwrap();
    sequence(&mut c, "f");
    assert!(c.path_copy_request.is_none());
    assert!(c.app.footer_workflow.is_none());
}

#[test]
fn first_background_listing_preserves_a_current_directory_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(root.path().into());
    app.launch_directory = Some(root.path().into());
    let mut c = Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    sequence(&mut c, "yc");
    c.app.complete_listing(
        0,
        1,
        Ok(fs::list(root.path(), &Default::default(), &AtomicBool::new(false)).unwrap()),
    );
    sequence(&mut c, "f");
    assert!(c.path_copy_request.is_some());
}

#[test]
fn stale_directory_before_activation_cannot_copy_a_replacement_context() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("pane");
    std::fs::create_dir(&directory).unwrap();
    let mut c = controller(&directory);
    std::fs::rename(&directory, root.path().join("old")).unwrap();
    std::fs::create_dir(&directory).unwrap();
    sequence(&mut c, "ycf");
    assert!(c.path_copy_request.is_none());
}

#[test]
fn relative_base_is_session_state_and_config_rejects_the_removed_preference() {
    let error = Settings::parse("[clipboard]\nrelative_path_base = 'opposite-pane'").unwrap_err();
    assert!(error.contains("unknown field"));
    assert!(
        !toml::to_string(&Settings::default())
            .unwrap()
            .contains("relative_path_base")
    );
}

#[test]
fn remapped_trigger_and_help_match_and_workflow_never_steals_an_editor() {
    use terminal_file_manager::input::{Action, HelpTarget, help_hits};
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    let settings = Settings::parse("[bindings]\ny = 'refresh'\nF11 = 'path-copy'").unwrap();
    c = Controller::new(c.app, Bindings::new(&settings.bindings).unwrap(), settings);
    assert!(
        help_hits(&c.app.help_rows, "F11")
            .iter()
            .any(|hit| hit.target == HelpTarget::Action(Action::PathCopy))
    );
    send(&mut c, KeyCode::F(11));
    assert!(c.app.footer_workflow.is_some());
    send(&mut c, KeyCode::Esc);
    send(&mut c, KeyCode::Char('/'));
    send(&mut c, KeyCode::F(11));
    assert!(c.app.footer_workflow.is_none());
    send(&mut c, KeyCode::Esc);
    c.app.help = true;
    send(&mut c, KeyCode::F(11));
    assert!(c.app.footer_workflow.is_none());
}

#[test]
fn unknown_modified_repeated_and_mouse_inputs_preserve_ownership_and_write_once() {
    use crossterm::event::KeyEventKind;
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    sequence(&mut c, "y");
    let bounds = Rect::new(0, 0, 120, 24);
    for (code, modifiers, kind) in [
        (KeyCode::Char('q'), KeyModifiers::NONE, KeyEventKind::Press),
        (
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Press,
        ),
        (KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Repeat),
        (KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release),
    ] {
        let g = ui::geometry(&c.app, bounds);
        c.handle(
            Event::Key(KeyEvent {
                code,
                modifiers,
                kind,
                state: crossterm::event::KeyEventState::NONE,
            }),
            &g,
            Default::default(),
        );
    }
    assert!(!c.quit);
    assert_eq!(c.app.footer_workflow.as_ref().unwrap().stage_index(), 0);
    for target in ["current", "full"] {
        click_choice(&mut c, target, bounds);
    }
    assert!(c.app.footer_workflow.is_none());
    assert!(c.path_copy_request.take().is_some());
    let g = ui::geometry(&c.app, bounds);
    let mut repeat = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
    repeat.kind = KeyEventKind::Repeat;
    c.handle(Event::Key(repeat), &g, Default::default());
    assert!(c.path_copy_request.is_none());
}

#[test]
fn navigation_and_modal_ownership_invalidate_before_next_frame_without_stale_callback() {
    let root = tempfile::tempdir().unwrap();
    let mut c = controller(root.path());
    sequence(&mut c, "yc");
    c.app.request_listing(0, root.path().join("elsewhere"));
    assert!(c.reconcile_footer());
    assert!(c.app.footer_workflow.is_none());
    assert!(c.path_copy_request.is_none());
    c.app.panes[0].pending = None;
    sequence(&mut c, "yc");
    c.app.popup = true;
    assert!(c.reconcile_footer());
    assert!(c.app.footer_workflow.is_none());
}

#[test]
fn filtered_marks_never_join_the_batch_and_control_names_reject_whole_batch() {
    let root = tempfile::tempdir().unwrap();
    for name in ["alpha", "beta", "界e\u{301}👩"] {
        std::fs::write(root.path().join(name), "").unwrap();
    }
    let mut c = controller(root.path());
    c.app.panes[0].select_all();
    c.app.panes[0].set_filter("beta".into());
    sequence(&mut c, "ymn");
    assert_eq!(c.path_copy_request.take().unwrap().text, "beta");
    c.app.panes[0].set_filter("界".into());
    sequence(&mut c, "yhn");
    assert_eq!(c.path_copy_request.take().unwrap().text, "界e\u{301}👩");
    #[cfg(unix)]
    {
        std::fs::write(root.path().join("bad\nname"), "").unwrap();
        let mut c = controller(root.path());
        c.app.panes[0].select_all();
        sequence(&mut c, "ymn");
        assert!(c.path_copy_request.is_none());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn non_unicode_native_entry_rejects_the_entire_marked_batch() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("valid"), "").unwrap();
    std::fs::write(
        root.path()
            .join(std::ffi::OsString::from_vec(vec![b'x', 0xff])),
        "",
    )
    .unwrap();
    let mut c = controller(root.path());
    c.app.panes[0].select_all();
    sequence(&mut c, "ymn");
    assert!(c.path_copy_request.is_none());
    assert!(c.app.footer_workflow.is_none());
}

#[test]
fn launch_base_is_stable_and_lexical_relative_paths_handle_roots_and_parent_segments() {
    use terminal_file_manager::path_copy::relative;
    let root = tempfile::tempdir().unwrap();
    let base = root.path().join("base");
    std::fs::create_dir(&base).unwrap();
    let mut c = controller(root.path());
    c.app.launch_directory = Some(base.clone());
    c.app.relative_path_base = base.clone();
    sequence(&mut c, "ycrk");
    assert_eq!(c.path_copy_request.take().unwrap().text, "..");
    assert_eq!(
        relative(&base.join("a/../b"), &base).unwrap(),
        Path::new("b")
    );
    assert_eq!(relative(&base, &base).unwrap(), Path::new("."));
    assert!(relative(Path::new("relative"), &base).is_err());
    #[cfg(unix)]
    {
        assert_eq!(
            relative(Path::new("/a/b"), Path::new("/a/c")).unwrap(),
            Path::new("../b")
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_relative_roots_drives_unc_and_verbatim_boundaries_are_native() {
    use terminal_file_manager::path_copy::relative;
    for (target, base, expected) in [
        (r"C:\a\b", r"c:\a", "b"),
        (r"\\server\share\dir", r"\\server\share\", "dir"),
        (r"\\?\C:\a", r"\\?\C:\", "a"),
    ] {
        assert_eq!(
            relative(Path::new(target), Path::new(base)).unwrap(),
            Path::new(expected)
        );
    }
    for (target, base) in [
        (r"D:\a", r"C:\a"),
        (r"\\server\share\a", r"\\server\other\a"),
        (r"C:a", r"C:\"),
    ] {
        assert!(relative(Path::new(target), Path::new(base)).is_err());
    }
}

#[cfg(unix)]
#[test]
fn symlink_name_and_full_path_never_copy_the_target_spelling() {
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("missing", root.path().join("link")).unwrap();
    let mut c = controller(root.path());
    sequence(&mut c, "yhn");
    assert_eq!(c.path_copy_request.take().unwrap().text, "link");
    sequence(&mut c, "yhf");
    assert_eq!(
        c.path_copy_request.take().unwrap().text,
        root.path().join("link").to_str().unwrap()
    );
}

fn click_choice(c: &mut Controller, target: &'static str, bounds: Rect) {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let g = ui::geometry(&c.app, bounds);
    let control = g
        .workflow
        .as_ref()
        .unwrap()
        .controls
        .iter()
        .find(|c| c.target == Some(terminal_file_manager::workflow::Target::Choice(target)))
        .unwrap();
    c.handle(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: control.area.x,
            row: control.area.y,
            modifiers: KeyModifiers::NONE,
        }),
        &g,
        Default::default(),
    );
}

#[path = "path_copy/session_base.rs"]
mod session_base;
