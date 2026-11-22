use super::*;
use crate::{app::App, input::Bindings, settings::Settings};

fn controller(settings: Settings) -> Controller {
    Controller::new(
        App::new(".".into()),
        Bindings::new(&settings.bindings).unwrap(),
        settings,
    )
}

#[test]
fn completed_no_popup_checks_create_config_and_preserve_omitted_replacement() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("nested/config.toml");
    let mut store = Some(Store::load(path.clone()).unwrap());
    let mut c = controller(Settings::default());
    c.check_compound_extensions("0.1.0").unwrap();
    assert!(c.app.compound_extension_review.is_none());
    assert!(!path.exists());
    persist(&mut c, &mut store).unwrap();
    let saved = Store::load(path).unwrap().settings;
    assert_eq!(
        saved.compound_extensions_checked_version.as_deref(),
        Some("0.1.0")
    );
    assert!(saved.compound_extensions.is_none());
    assert!(!c.compound_check_save_pending && !c.preferences_changed);
}

#[test]
fn completed_decisions_save_immediately_and_reloads_suppress_the_same_version() {
    use crate::ui;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    for code in [KeyCode::Enter, KeyCode::Esc] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.toml");
        std::fs::write(&path, "sidebar = true\ncompound_extensions = ['.user.js']").unwrap();
        let mut store = Some(Store::load(path.clone()).unwrap());
        let mut c = controller(store.as_ref().unwrap().settings.clone());
        c.check_compound_extensions("0.1.0").unwrap();
        persist(&mut c, &mut store).unwrap();
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("checked_version")
        );
        let g = ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 120, 40));
        c.handle(
            Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            &g,
            std::time::Duration::ZERO,
        );
        persist(&mut c, &mut store).unwrap();
        let mut restarted = controller(Store::load(path).unwrap().settings);
        assert!(restarted.preferences.sidebar);
        assert_eq!(
            restarted.preferences.compound_extensions,
            Some(vec![".user.js".into()])
        );
        restarted.check_compound_extensions("0.1.0").unwrap();
        assert!(restarted.app.compound_extension_review.is_none());
        assert!(!restarted.compound_check_save_pending);
        restarted.check_compound_extensions("0.2.0").unwrap();
        assert!(restarted.app.compound_extension_review.is_some());
    }
}

#[test]
fn skip_does_not_write_or_downgrade_a_newer_marker() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    let bytes = "# keep this file untouched\ncompound_extensions_checked_version = '9.0.0'\n";
    std::fs::write(&path, bytes).unwrap();
    let mut store = Some(Store::load(path.clone()).unwrap());
    let mut c = controller(store.as_ref().unwrap().settings.clone());
    c.check_compound_extensions("0.1.0").unwrap();
    persist(&mut c, &mut store).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), bytes);
    assert_eq!(
        c.preferences.compound_extensions_checked_version.as_deref(),
        Some("9.0.0")
    );
}

#[test]
fn save_failures_are_visible_and_do_not_overwrite_the_existing_config() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    std::fs::create_dir(&path).unwrap();
    let mut store = Some(Store {
        path: path.clone(),
        settings: Settings::default(),
    });
    let mut c = controller(Settings::default());
    c.check_compound_extensions("0.1.0").unwrap();
    assert!(
        persist(&mut c, &mut None)
            .unwrap_err()
            .contains("no user config directory")
    );
    assert!(
        persist(&mut c, &mut store)
            .unwrap_err()
            .contains("regular file")
    );
    assert!(path.is_dir());
    assert!(c.compound_check_save_pending);
}

#[test]
fn approved_replacement_and_cleanup_survive_store_reload_without_materializing_defaults() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    std::fs::write(&path, "sidebar_left = true\ncompound_extensions = []\ncompound_extensions_extend = ['.tar.gz', '.user.js']").unwrap();
    let mut store = Some(Store::load(path.clone()).unwrap());
    let mut c = controller(store.as_ref().unwrap().settings.clone());
    c.check_compound_extensions("0.1.0").unwrap();
    for (index, code) in [KeyCode::Char('d'), KeyCode::Char('r')]
        .into_iter()
        .enumerate()
    {
        let g = crate::ui::geometry(&c.app, ratatui::layout::Rect::new(0, 0, 120, 40));
        c.handle(
            Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            &g,
            std::time::Duration::ZERO,
        );
        persist(&mut c, &mut store).unwrap();
        if index == 0 {
            let pending = Store::load(path.clone()).unwrap().settings;
            assert!(pending.compound_extensions_checked_version.is_none());
            assert_eq!(pending.compound_extensions, Some(vec![]));
        }
    }
    assert!(!c.quit);
    let saved = Store::load(path).unwrap().settings;
    assert!(saved.compound_extensions.is_none());
    assert_eq!(saved.compound_extensions_extend, [".user.js"]);
    assert_eq!(
        saved.compound_extensions_checked_version.as_deref(),
        Some("0.1.0")
    );
    assert!(saved.sidebar_left);
}
