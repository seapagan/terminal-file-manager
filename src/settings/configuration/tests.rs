use super::*;
use crate::settings::Store;
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
    let mut store = Configuration::load(Some(path.clone())).unwrap();
    let mut c = controller(Settings::default());
    c.check_compound_extensions("0.1.0").unwrap();
    assert!(c.app.compound_extension_review.is_none());
    assert!(!path.exists());
    store.synchronize(&mut c);
    let saved = Store::load(path).unwrap().settings;
    assert_eq!(
        saved.compound_extensions_checked_version.as_deref(),
        Some("0.1.0")
    );
    assert!(saved.compound_extensions.is_none());
    assert!(!store.is_unsaved() && !c.preferences_changed);
}

#[test]
fn completed_decisions_save_immediately_and_reloads_suppress_the_same_version() {
    use crate::ui;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    for code in [KeyCode::Enter, KeyCode::Esc] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.toml");
        std::fs::write(&path, "sidebar = true\ncompound_extensions = ['.user.js']").unwrap();
        let mut store = Configuration::load(Some(path.clone())).unwrap();
        let mut c = controller(store.preferences());
        c.check_compound_extensions("0.1.0").unwrap();
        store.synchronize(&mut c);
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
        store.synchronize(&mut c);
        let mut restarted = controller(Store::load(path).unwrap().settings);
        assert!(restarted.preferences.sidebar);
        assert_eq!(
            restarted.preferences.compound_extensions,
            Some(vec![".user.js".into()])
        );
        restarted.check_compound_extensions("0.1.0").unwrap();
        assert!(restarted.app.compound_extension_review.is_none());
        assert!(!restarted.preferences_changed);
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
    let mut store = Configuration::load(Some(path.clone())).unwrap();
    let mut c = controller(store.preferences());
    c.check_compound_extensions("0.1.0").unwrap();
    store.synchronize(&mut c);
    assert_eq!(std::fs::read_to_string(path).unwrap(), bytes);
    assert_eq!(
        c.preferences.compound_extensions_checked_version.as_deref(),
        Some("9.0.0")
    );
}

#[test]
fn approved_replacement_and_cleanup_survive_store_reload_without_materializing_defaults() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    std::fs::write(&path, "sidebar_left = true\ncompound_extensions = []\ncompound_extensions_extend = ['.tar.gz', '.user.js']").unwrap();
    let mut store = Configuration::load(Some(path.clone())).unwrap();
    let mut c = controller(store.preferences());
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
        store.synchronize(&mut c);
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
