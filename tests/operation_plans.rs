use std::{fs, path::PathBuf};
use terminal_file_manager::{
    app::{App, IgnoreMode},
    fs::IgnoreContext,
    operations::{Coordinator, Operation, Plan},
};

fn browser(root: &std::path::Path) -> App {
    let mut app = App::new(root.into());
    let snapshot =
        terminal_file_manager::fs::list(root, &IgnoreContext::default(), &false.into()).unwrap();
    app.complete_listing(0, 1, Ok(snapshot));
    app
}

#[test]
fn submission_freezes_visible_operands_and_destination() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("alpha"), "a").unwrap();
    fs::write(root.path().join("beta"), "b").unwrap();
    let mut app = browser(root.path());
    app.panes[0].select_all();
    app.panes[0].set_filter("alpha".into());
    let destination = root.path().join("target");
    let plan = Plan::capture(&app.panes[0], Operation::Copy, Some(destination.clone())).unwrap();
    app.panes[0].set_filter("beta".into());
    app.panes[0].clear_visible();
    app.panes[0].path = PathBuf::from("elsewhere");
    assert_eq!(plan.operands.len(), 1);
    assert_eq!(plan.operands[0].path, root.path().join("alpha"));
    assert_eq!(plan.destination, Some(destination));
}

#[test]
fn empty_pending_unknown_ignore_and_replaced_marks_cannot_authorize() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("alpha"), "a").unwrap();
    let mut app = browser(root.path());
    app.panes[0].set_filter("no matches".into());
    assert!(Plan::capture(&app.panes[0], Operation::Delete, None).is_err());
    app.panes[0].set_filter(String::new());
    app.panes[0].ignore = IgnoreMode::Hide;
    app.panes[0].entries[0].ignored = None;
    assert!(Plan::capture(&app.panes[0], Operation::Delete, None).is_err());
    app.panes[0].ignore = IgnoreMode::Off;
    app.panes[0].select_all();
    app.panes[0].entries[0].identity.as_mut().unwrap().object = Some((99, 99));
    assert!(Plan::capture(&app.panes[0], Operation::Delete, None).is_err());
    app.panes[0].marks.clear();
    app.request_listing(0, root.path().into());
    assert!(Plan::capture(&app.panes[0], Operation::Delete, None).is_err());
}

#[test]
fn coordinator_limits_jobs_and_holds_all_overlapping_claims_atomically() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a", "b", "c"] {
        fs::create_dir(root.path().join(name)).unwrap();
    }
    let mut coordinator = Coordinator::default();
    let a = Plan::paths(Operation::Delete, vec![root.path().join("a")], None).unwrap();
    let b = Plan::paths(Operation::Delete, vec![root.path().join("b")], None).unwrap();
    let c = Plan::paths(Operation::Delete, vec![root.path().join("c")], None).unwrap();
    assert!(coordinator.acquire(1, &a).unwrap());
    assert!(!coordinator.acquire(2, &a).unwrap());
    assert!(coordinator.acquire(3, &b).unwrap());
    assert!(!coordinator.acquire(4, &c).unwrap());
    coordinator.release(1);
    assert!(coordinator.acquire(4, &c).unwrap());
    assert_eq!(coordinator.active(), 2);
}

#[test]
fn source_aliases_and_ancestor_writes_conflict_but_independent_reads_share() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("tree")).unwrap();
    fs::write(root.path().join("tree/file"), "data").unwrap();
    fs::hard_link(root.path().join("tree/file"), root.path().join("alias")).unwrap();
    fs::create_dir(root.path().join("dest1")).unwrap();
    fs::create_dir(root.path().join("dest2")).unwrap();
    let plan = |source: &str, destination: &str| {
        Plan::paths(
            Operation::Copy,
            vec![root.path().join(source)],
            Some(root.path().join(destination)),
        )
        .unwrap()
    };
    let mut coordinator = Coordinator::default();
    assert!(coordinator.acquire(1, &plan("tree/file", "dest1")).unwrap());
    assert!(coordinator.acquire(2, &plan("alias", "dest2")).unwrap());
    coordinator.release(2);
    let remove = Plan::paths(Operation::Delete, vec![root.path().join("tree")], None).unwrap();
    assert!(!coordinator.acquire(3, &remove).unwrap());
    let alias = Plan::paths(Operation::Delete, vec![root.path().join("alias")], None).unwrap();
    assert!(!coordinator.acquire(4, &alias).unwrap());
}
