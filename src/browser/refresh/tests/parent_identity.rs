//! Parent installation and cursor restoration have independent completion paths.
use super::*;

#[test]
fn parent_navigation_completes_while_exited_identity_is_blocked() {
    for outcome in [
        LookupOutcome::Restore,
        LookupOutcome::Moved,
        LookupOutcome::Missing,
    ] {
        assert_parent_result_while_identity_is_blocked(outcome);
    }
}

enum LookupOutcome {
    Restore,
    Moved,
    Missing,
}

fn assert_parent_result_while_identity_is_blocked(outcome: LookupOutcome) {
    let (root, mut controller) = startup_child();
    let child = root.path().join("child");
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let parent = controller.app.parent().unwrap();
    let (started, release, cancelled) =
        hold_request(&mut pool, parent.generation, RequestKind::ParentIdentity);
    pool.submit(parent);
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    collect_until(&mut controller, &mut pool, |app| {
        app.panes[0].local_path() == root.path()
    });
    let pane = &mut controller.app.panes[0];
    assert!(!pane.listing);
    assert_eq!(pane.entries[0].native_path(), Some(&child));
    assert!(pane.parent_focused());
    assert_eq!(pool.active_count(), 1);
    match outcome {
        LookupOutcome::Moved => {
            pane.move_cursor(1, 20);
            pane.move_cursor(-1, 20);
            assert!(pane.parent_focused());
        }
        LookupOutcome::Missing => std::fs::remove_dir(&child).unwrap(),
        LookupOutcome::Restore => {}
    }
    release.send(()).unwrap();
    assert!(!cancelled.recv_timeout(Duration::from_secs(10)).unwrap());
    drain_stale_listing(&mut controller, &mut pool);
    let pane = &controller.app.panes[0];
    assert!(pane.error.is_none());
    if matches!(outcome, LookupOutcome::Restore) {
        assert_eq!(pane.cursor, Some(child));
    } else {
        assert!(pane.parent_focused());
    }
}
