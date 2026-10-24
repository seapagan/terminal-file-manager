//! Pending worker refreshes retain the selected listing snapshot, never the previous entry.
use super::{tests::metadata_text, *};
use crate::{
    fs,
    input::{Bindings, Controller},
};

fn loaded() -> (tempfile::TempDir, Controller) {
    let root = tempfile::tempdir().unwrap();
    let mut entries = Vec::new();
    for (name, content) in [("alpha", "1"), ("bravo", "22"), ("cider", "333")] {
        let path = root.path().join(name);
        std::fs::write(&path, content).unwrap();
        entries.push(fs::inspect(&path));
        std::fs::remove_file(path).unwrap();
    }
    let mut app = App::new(root.path().into());
    for id in 0..2 {
        app.complete_listing(
            id,
            1,
            Ok(fs::Snapshot {
                directory: Some(Box::new(fs::inspect(root.path()))),
                path: root.path().into(),
                entries: entries.clone(),
                errors: vec![],
            }),
        );
        app.panes[id].sidebar = true;
    }
    app.panes[1].move_cursor(2, 20);
    app.sidebar = true;
    (
        root,
        Controller::new(
            app,
            Bindings::new(&Default::default()).unwrap(),
            Default::default(),
        ),
    )
}

fn action(c: &mut Controller, action: Action) {
    let geometry = crate::ui::geometry(&c.app, Rect::new(0, 0, 180, 40));
    c.dispatch(action, &geometry);
}

#[test]
fn stale_directory_views_show_the_error_instead_of_cached_metadata_or_totals() {
    let (_root, mut c) = loaded();
    c.app.request_metadata(0).unwrap();
    let refresh = c.app.request_refresh(0).unwrap();
    let snapshot = fs::Snapshot {
        directory: None,
        path: c.app.panes[0].path.clone(),
        entries: vec![],
        errors: vec![],
    };
    c.app.complete_listing(0, refresh.generation, Ok(snapshot));
    for floating in [false, true] {
        let text = metadata_text(&c.app, floating);
        assert!(text.contains("Stale directory listing"), "{text}");
        assert!(text.contains("identity"), "{text}");
        assert!(!text.contains("Canonical:") && !text.contains("Descendants:"));
    }
}

fn assert_cached_fields(text: &str, name: &str, size: &str) {
    assert!(
        text.contains(name) && text.contains(size) && text.contains("File"),
        "{text}"
    );
    for label in [
        "Name:",
        "Path:",
        "Canonical:",
        "Type:",
        "Size:",
        "Modified:",
        "Attributes:",
        "Owner/group:",
    ] {
        assert!(text.contains(label), "{text}");
    }
    assert!(!text.contains("Loading") && !text.contains("Error:"));
}

#[test]
fn navigation_renders_cached_fields_before_immediate_worker_completions() {
    for floating in [false, true] {
        let (_root, mut c) = loaded();
        c.app.popup = floating;
        let first = c.app.request_metadata(0).unwrap();
        let first_entry = c.app.panes[0].current().unwrap().clone();
        for (name, size) in [("alpha", "1 B"), ("bravo", "2 B"), ("cider", "3 B")] {
            let request = if name == "alpha" {
                first.clone()
            } else {
                c.app.request_metadata(0).unwrap()
            };
            let pending = metadata_text(&c.app, floating);
            assert_cached_fields(&pending, name, size);
            let entry = c.app.panes[0].current().unwrap().clone();
            assert!(c.app.complete_metadata(&request, entry, None));
            assert!(pending.contains("Resolving…"));
            let finished = metadata_text(&c.app, floating);
            assert_cached_fields(&finished, name, size);
            assert!(finished.contains("Unavailable") && !finished.contains("Resolving…"));
            if name != "alpha" {
                assert!(!c.app.complete_metadata(
                    &first,
                    first_entry.clone(),
                    Some("OLD-CANONICAL".into())
                ));
            }
            if name != "cider" {
                if floating {
                    action(&mut c, Action::Metadata);
                }
                action(&mut c, Action::Down);
                if floating {
                    action(&mut c, Action::Metadata);
                }
            }
        }
    }
}

#[test]
fn rapid_pending_selections_reject_stale_results_and_clear_empty_selection() {
    for floating in [false, true] {
        let (_root, mut c) = loaded();
        let first = c.app.request_metadata(0).unwrap();
        let old = c.app.panes[0].current().unwrap().clone();
        let mut latest = first.clone();
        for (name, size) in [("bravo", "2 B"), ("cider", "3 B")] {
            action(&mut c, Action::Down);
            latest = c.app.request_metadata(0).unwrap();
            assert_cached_fields(&metadata_text(&c.app, floating), name, size);
            assert!(metadata_text(&c.app, floating).contains("Resolving…"));
            assert!(
                !c.app
                    .complete_metadata(&first, old.clone(), Some("OLD-CANONICAL".into()))
            );
        }
        assert!(!c.app.complete_failure(&first, "stale failure".into()));
        let current = c.app.panes[0].current().unwrap().clone();
        assert!(c.app.complete_metadata(&latest, current, None));
        assert_cached_fields(&metadata_text(&c.app, floating), "cider", "3 B");
        assert!(metadata_text(&c.app, floating).contains("Unavailable"));
        c.app.panes[0].set_filter("no-match".into());
        assert!(c.app.request_metadata(0).is_none());
        let empty = metadata_text(&c.app, floating);
        assert!(
            empty.contains("No visible entry")
                && !empty.contains("cider")
                && !empty.contains("Loading")
        );
    }
}

#[test]
fn delayed_refresh_clears_old_canonical_and_preserves_errors_without_loading() {
    let (_root, mut c) = loaded();
    let first = c.app.request_metadata(0).unwrap();
    let old = c.app.panes[0].current().unwrap().clone();
    c.app
        .complete_metadata(&first, old.clone(), Some("OLD-CANONICAL".into()));
    action(&mut c, Action::Down);
    let next = c.app.request_metadata(0).unwrap();
    for floating in [false, true] {
        let text = metadata_text(&c.app, floating);
        assert!(text.contains("bravo"), "{text}");
        assert!(
            text.split("Canonical:")
                .nth(1)
                .unwrap()
                .trim_start()
                .starts_with("Resolving…")
        );
        assert!(
            !text.contains("alpha") && !text.contains("OLD-CANONICAL") && !text.contains("Loading")
        );
    }
    assert!(
        !c.app
            .complete_metadata(&first, old, Some("OLD-CANONICAL".into()))
    );
    assert!(c.app.complete_failure(&next, "refresh denied".into()));
    for floating in [false, true] {
        let text = metadata_text(&c.app, floating);
        assert!(text.contains("bravo") && text.contains("refresh denied"));
        assert!(!text.contains("Loading") && !text.contains("OLD-CANONICAL"));
        assert!(text.contains("Unavailable") && !text.contains("Resolving…"));
    }
}

#[test]
fn immediate_refresh_fills_canonical_without_a_loading_transition() {
    let (_root, mut c) = loaded();
    let request = c.app.request_metadata(0).unwrap();
    let entry = c.app.panes[0].current().unwrap().clone();
    assert!(
        c.app
            .complete_metadata(&request, entry, Some("CURRENT-CANONICAL".into()))
    );
    for floating in [false, true] {
        let text = metadata_text(&c.app, floating);
        assert!(text.contains("alpha") && text.contains("CURRENT-CANONICAL"));
        assert!(!text.contains("Loading"));
    }
}

#[test]
fn pending_metadata_and_errors_stay_pane_local_with_independent_surface_formats() {
    let (_root, mut c) = loaded();
    c.app.popup = true;
    c.app.sync_metadata_views(true);
    c.app.metadata_formats[0]
        .as_mut()
        .unwrap()
        .toggle(Action::Permissions);
    let sidebar_format = c.app.metadata_format(false);
    let popup_format = c.app.metadata_format(true);
    let first = c.app.request_metadata(0).unwrap();
    assert_eq!(c.app.metadata_format(false), sidebar_format);
    assert_eq!(c.app.metadata_format(true), popup_format);
    for a in [Action::TogglePanes, Action::FocusPane, Action::TogglePanes] {
        action(&mut c, a);
    }
    let other = c.app.request_metadata(1).unwrap();
    assert!(c.app.complete_failure(&first, "pane-zero-only".into()));
    for floating in [false, true] {
        let text = metadata_text(&c.app, floating);
        assert!(
            text.contains("cider") && !text.contains("alpha") && !text.contains("pane-zero-only")
        );
    }
    let entry = c.app.panes[1].current().unwrap().clone();
    assert!(c.app.complete_metadata(&other, entry, None));
    assert!(metadata_text(&c.app, false).contains("cider"));
    c.app.active = 0;
    assert!(metadata_text(&c.app, true).contains("pane-zero-only"));
}

#[test]
fn canonical_resolution_distinguishes_native_success_and_failure_in_both_views() {
    let (_root, mut c) = loaded();
    let request = c.app.request_metadata(0).unwrap();
    let cached = c.app.panes[0].current().unwrap().clone();
    // The listing fixture removed the file after caching its metadata.
    assert!(std::fs::canonicalize(&request.path).is_err());
    for floating in [false, true] {
        let text = metadata_text(&c.app, floating);
        assert_cached_fields(&text, "alpha", "1 B");
        assert!(text.contains("Resolving…"));
    }
    let canonical = std::fs::canonicalize(&request.path).ok();
    assert!(c.app.complete_metadata(&request, cached, canonical));
    for floating in [false, true] {
        let text = metadata_text(&c.app, floating);
        assert_cached_fields(&text, "alpha", "1 B");
        assert!(text.contains("Unavailable") && !text.contains("Resolving…"));
    }
    action(&mut c, Action::Down);
    let request = c.app.request_metadata(0).unwrap();
    std::fs::write(&request.path, "22").unwrap();
    let canonical = std::fs::canonicalize(&request.path).unwrap();
    let entry = fs::inspect(&request.path);
    assert!(
        c.app
            .complete_metadata(&request, entry, Some(canonical.clone()))
    );
    for floating in [false, true] {
        let text = metadata_text(&c.app, floating);
        assert_cached_fields(&text, "bravo", "2 B");
        assert!(text.contains(&canonical.display().to_string()));
        assert!(!text.contains("Resolving…") && !text.contains("Unavailable"));
    }
}

#[test]
fn returning_to_a_path_rejects_its_previous_canonical_generation_in_both_views() {
    let (_root, mut c) = loaded();
    let first = c.app.request_metadata(0).unwrap();
    let entry = c.app.panes[0].current().unwrap().clone();
    action(&mut c, Action::Down);
    c.app.request_metadata(0).unwrap();
    action(&mut c, Action::Up);
    let latest = c.app.request_metadata(0).unwrap();
    assert_eq!(first.path, latest.path);
    assert_ne!(first.generation, latest.generation);
    assert!(
        !c.app
            .complete_metadata(&first, entry.clone(), Some("STALE".into()))
    );
    for floating in [false, true] {
        let text = metadata_text(&c.app, floating);
        assert_cached_fields(&text, "alpha", "1 B");
        assert!(text.contains("Resolving…") && !text.contains("STALE"));
    }
    assert!(c.app.complete_metadata(&latest, entry.clone(), None));
    assert!(!c.app.complete_metadata(&first, entry, Some("STALE".into())));
    assert!(!c.app.complete_failure(&first, "stale failure".into()));
    for floating in [false, true] {
        let text = metadata_text(&c.app, floating);
        assert_cached_fields(&text, "alpha", "1 B");
        assert!(text.contains("Unavailable") && !text.contains("STALE"));
        assert!(!text.contains("Resolving…"));
    }
}
