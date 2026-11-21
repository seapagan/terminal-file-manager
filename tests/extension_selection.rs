//! Extension marking uses native names and only replaces the visible working set.
use crossterm::event::{Event, KeyEvent};
use std::{path::Path, time::Duration};
use terminal_file_manager::{
    app::App,
    fs,
    input::{Bindings, Controller, chord},
    settings::Settings,
    ui,
};

fn loaded(path: &Path) -> Controller {
    let mut app = App::new(path.into());
    let snapshot = fs::list(path, &fs::IgnoreContext::isolated(), &Default::default()).unwrap();
    app.complete_listing(0, 1, Ok(snapshot));
    Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Settings::default(),
    )
}
fn key(c: &mut Controller, text: &str) {
    let (code, modifiers) = chord(text).unwrap();
    let g = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 120, 24));
    assert!(
        c.handle(
            Event::Key(KeyEvent::new(code, modifiers)),
            &g,
            Duration::ZERO
        )
        .is_empty()
    );
}
fn marks(c: &Controller) -> Vec<String> {
    let mut names: Vec<_> = c.app.panes[0]
        .marks
        .keys()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}
#[test]
fn exact_and_final_extension_actions_distinguish_configured_compounds() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "a.tar.gz",
        "b.tar.gz",
        "plain.gz",
        "notes.final.pdf",
        "other.pdf",
        "no-extension",
    ] {
        std::fs::write(root.path().join(name), "owned").unwrap();
    }
    std::fs::create_dir(root.path().join("directory.tar.gz")).unwrap();
    let mut c = loaded(root.path());
    c.app.panes[0].cursor = Some(root.path().join("a.tar.gz"));
    key(&mut c, "Alt+e");
    assert_eq!(marks(&c), ["a.tar.gz", "b.tar.gz"]);
    key(&mut c, "Alt+Shift+e");
    assert_eq!(marks(&c), ["a.tar.gz", "b.tar.gz", "plain.gz"]);
    c.app.panes[0].cursor = Some(root.path().join("plain.gz"));
    key(&mut c, "Alt+e");
    assert_eq!(marks(&c), ["plain.gz"]);
    c.app.panes[0].cursor = Some(root.path().join("notes.final.pdf"));
    key(&mut c, "Alt+e");
    assert_eq!(marks(&c), ["notes.final.pdf", "other.pdf"]);
    c.app.panes[0].cursor = Some(root.path().join("no-extension"));
    key(&mut c, "Alt+e");
    assert_eq!(marks(&c), ["notes.final.pdf", "other.pdf"]);
    assert!(c.app.operations.outbox.is_empty());
}

#[test]
fn filtered_and_hidden_marks_survive_and_invalid_cursors_do_not_expand_scope() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a.rs", "b.rs", "other.log", ".hidden.rs"] {
        std::fs::write(root.path().join(name), "owned").unwrap();
    }
    let mut c = loaded(root.path());
    c.app.panes[0].show_hidden = true;
    c.app.panes[0].refilter();
    c.app.panes[0].select_all();
    c.app.panes[0].show_hidden = false;
    c.app.panes[0].set_filter(".rs".into());
    c.app.panes[0].cursor = Some(root.path().join("a.rs"));
    key(&mut c, "Alt+e");
    assert_eq!(marks(&c), [".hidden.rs", "a.rs", "b.rs", "other.log"]);
    c.app.panes[0].cursor = Some(root.path().join("absent.rs"));
    key(&mut c, "Alt+e");
    assert_eq!(marks(&c).len(), 4);
    c.app.panes[0].cursor = root.path().parent().map(Path::to_owned);
    key(&mut c, "Alt+Shift+e");
    assert_eq!(marks(&c).len(), 4);
}

#[test]
fn custom_compounds_use_longest_suffix_and_strict_configuration() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a.test.js", "b.test.js", "plain.js", "c.long.test.js"] {
        std::fs::write(root.path().join(name), "owned").unwrap();
    }
    let mut c = loaded(root.path());
    c.preferences = Settings::parse("compound_extensions = ['.test.js', '.long.test.js']").unwrap();
    c.app.panes[0].cursor = Some(root.path().join("a.test.js"));
    key(&mut c, "Alt+e");
    assert_eq!(marks(&c), ["a.test.js", "b.test.js"]);
    key(&mut c, "Alt+Shift+e");
    assert_eq!(marks(&c).len(), 4);
    for suffix in ["rs", ".", ".a/rs", ".a\\rs"] {
        assert!(Settings::parse(&format!("compound_extensions = ['{suffix}']")).is_err());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn invalid_native_extension_bytes_never_match_the_lossy_replacement_character() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join(OsString::from_vec(b"a.\xff".to_vec()));
    let b = root.path().join(OsString::from_vec(b"b.\xff".to_vec()));
    let different = root.path().join("c.�");
    for path in [&a, &b, &different] {
        std::fs::write(path, "owned").unwrap();
    }
    let mut c = loaded(root.path());
    c.app.panes[0].cursor = Some(a.clone());
    key(&mut c, "Alt+e");
    assert!(c.app.panes[0].marks.contains_key(&a));
    assert!(c.app.panes[0].marks.contains_key(&b));
    assert!(!c.app.panes[0].marks.contains_key(&different));
}
