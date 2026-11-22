use super::*;
use terminal_file_manager::{
    fs::{self, IgnoreContext},
    view::ViewSettings,
};

fn listed(root: &std::path::Path) -> Controller {
    std::fs::write(root.join("item"), "owned").unwrap();
    let mut app = App::new(root.into());
    let snapshot = fs::list(root, &IgnoreContext::isolated(), &false.into()).unwrap();
    app.complete_listing(0, 1, Ok(snapshot));
    app.panes[0].cursor = Some(root.join("item"));
    Controller::new(
        app,
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    )
}

#[test]
fn popup_shortcuts_do_not_escape_editors_workflows_or_existing_modals() {
    let bounds = Rect::new(0, 0, 120, 30);
    for surface in [
        "?",
        "Alt+Enter",
        "V",
        "Alt+v",
        "Alt+b",
        "/",
        "Alt+l",
        "y",
        "Alt+h",
        "F7",
        "F9",
    ] {
        for shortcut in ["S", "G", "H", "I"] {
            let root = tempfile::tempdir().unwrap();
            let mut c = listed(root.path());
            key(&mut c, surface, bounds);
            let before = c.app.panes.each_ref().map(ViewSettings::capture);
            key(&mut c, shortcut, bounds);
            assert!(c.app.policy_menu.is_none(), "{surface}: {shortcut}");
            assert_eq!(c.app.panes.each_ref().map(ViewSettings::capture), before);
            assert!(c.app.operations.outbox.is_empty());
            assert!(!c.preferences_changed);
            assert_eq!(std::fs::read(root.path().join("item")).unwrap(), b"owned");
        }
    }
}
