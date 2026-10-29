use super::*;
use crate::{
    app::App,
    color::Theme,
    fs::{IgnoreContext, list},
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

fn loaded() -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    for i in 0..2000 {
        std::fs::write(root.path().join(format!("file-{i}")), "x").unwrap();
    }
    let mut app = App::new(root.path().into());
    app.complete_listing(
        0,
        1,
        Ok(list(root.path(), &IgnoreContext::isolated(), &false.into()).unwrap()),
    );
    let mut state = PatternSelection::new(&app.panes[0], 0);
    state.set_expression("*".into());
    app.pattern = Some(state);
    (root, app)
}

#[test]
fn unchanged_geometry_does_not_rescan_and_edits_and_pane_changes_invalidate() {
    let (_root, mut app) = loaded();
    for width in [80, 120, 40, 80] {
        ui::geometry(&app, Rect::new(0, 0, width, 24));
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
        terminal
            .draw(|f| {
                ui::render(f, &app, &Theme::new(colored_text::ColorLevel::NoColor));
            })
            .unwrap();
    }
    assert_eq!(app.pattern.as_ref().unwrap().preview_scans.get(), 1);
    app.pattern.as_mut().unwrap().toggle_case();
    ui::geometry(&app, Rect::new(0, 0, 80, 24));
    assert_eq!(app.pattern.as_ref().unwrap().preview_scans.get(), 2);
    app.panes[0].select_all();
    ui::geometry(&app, Rect::new(0, 0, 80, 24));
    assert_eq!(app.pattern.as_ref().unwrap().preview_scans.get(), 3);
    app.panes[0].set_filter("missing".into());
    ui::geometry(&app, Rect::new(0, 0, 80, 24));
    assert_eq!(app.pattern.as_ref().unwrap().preview_scans.get(), 4);
}

#[test]
fn preview_invalidates_for_expression_mode_sort_refresh_and_live_apply() {
    let (root, mut app) = loaded();
    let mut state = app.pattern.take().unwrap();
    assert_eq!(state.cached_preview(&app.panes[0]).unwrap().matches, 2000);
    state.set_expression("file-1".into());
    assert_eq!(state.cached_preview(&app.panes[0]).unwrap().matches, 1);
    state.set_mode(PatternMode::Regex);
    assert!(state.cached_preview(&app.panes[0]).unwrap().matches > 1);
    let scans = state.preview_scans.get();
    app.panes[0].sort.reverse = true;
    app.panes[0].resort();
    assert!(state.cached_preview(&app.panes[0]).is_ok());
    assert_eq!(state.preview_scans.get(), scans + 1);
    let request = app.request_refresh(0).unwrap();
    assert!(state.cached_preview(&app.panes[0]).is_err());
    app.complete_listing(
        0,
        request.generation,
        Ok(list(root.path(), &IgnoreContext::isolated(), &false.into()).unwrap()),
    );
    assert!(state.cached_preview(&app.panes[0]).is_ok());
    let cached = state.preview_scans.get();
    app.panes[0].entries[0].identity.as_mut().unwrap().object = Some((u64::MAX, u64::MAX));
    assert!(
        state
            .apply(&mut app.panes[0], PatternOperation::Mark)
            .is_err()
    );
    assert_eq!(state.preview_scans.get(), cached + 1);
    assert!(app.panes[0].marks.is_empty());
}

#[test]
fn invalid_range_target_preserves_revision_and_cache_but_real_ranges_invalidate() {
    let (_root, mut app) = loaded();
    let state = app.pattern.take().unwrap();
    let pane = &mut app.panes[0];
    assert_eq!(state.cached_preview(pane).unwrap().marked, 0);
    let revision = pane.selection_revision;
    let cursor = pane.cursor.clone();
    pane.select_range_to(pane.path.join("nonexistent"), 10);
    assert_eq!(pane.selection_revision, revision);
    assert_eq!(pane.cursor, cursor);
    assert!(pane.marks.is_empty());
    assert_eq!(state.cached_preview(pane).unwrap().marked, 0);
    assert_eq!(state.preview_scans.get(), 1);
    let target = pane.entries[pane.visible[1]].path.clone();
    pane.select_range_to(target, 10);
    assert_ne!(pane.selection_revision, revision);
    assert_eq!(state.cached_preview(pane).unwrap().marked, 2);
    assert_eq!(state.preview_scans.get(), 2);
    let revision = pane.selection_revision;
    pane.select_range_to(pane.path.join("nonexistent"), 10);
    assert_eq!(pane.selection_revision, revision);
    assert_eq!(state.cached_preview(pane).unwrap().marked, 2);
    assert_eq!(state.preview_scans.get(), 2);
    pane.extend_range(-1, 10);
    assert_ne!(pane.selection_revision, revision);
    assert_eq!(state.cached_preview(pane).unwrap().marked, 1);
    assert_eq!(state.preview_scans.get(), 3);
}
