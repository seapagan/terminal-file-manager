use super::*;
use terminal_file_manager::{
    app::IgnoreMode,
    fs::{IgnoreContext, list},
};

#[test]
fn choosing_ignore_off_never_hides_the_selected_ignored_entry() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a-visible"), "owned").unwrap();
    std::fs::write(root.path().join("z-ignored"), "owned").unwrap();
    let mut snapshot = list(root.path(), &IgnoreContext::isolated(), &false.into()).unwrap();
    snapshot
        .entries
        .iter_mut()
        .find(|e| e.name == "z-ignored")
        .unwrap()
        .ignored = Some(true);
    let mut c = Controller::new(
        App::new(root.path().into()),
        Bindings::new(&Default::default()).unwrap(),
        Default::default(),
    );
    c.app.complete_listing(0, 1, Ok(snapshot));
    c.app.panes[0].cursor = Some(root.path().join("z-ignored"));
    c.app.panes[0].profile = Some("Detailed".into());
    let bounds = Rect::new(0, 0, 120, 30);
    let g = ui::geometry(&c.app, bounds);
    let control = g
        .policy_controls
        .iter()
        .find(|c| c.action == Action::Ignore)
        .unwrap();
    click(&mut c, control.area.x, control.area.y, bounds);
    key(&mut c, "End", bounds);
    key(&mut c, "Enter", bounds);
    assert_eq!(c.app.panes[0].ignore, IgnoreMode::Off);
    assert_eq!(
        c.app.panes[0].cursor.as_deref(),
        Some(root.path().join("z-ignored").as_path())
    );
    assert_eq!(c.preferences.ignore, IgnoreMode::Off);
    assert!(c.preferences_changed && c.app.panes[0].profile_modified);
    assert_eq!(c.app.panes[1].ignore, IgnoreMode::Dim);
}
