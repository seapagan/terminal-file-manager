use super::*;
fn git(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", root.join("no-global-config"))
        .env("GIT_CONFIG_SYSTEM", root.join("no-system-config"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn native_git_create_rename_delete_burst_converges_without_losing_scope() {
    let root = tempfile::tempdir().unwrap();
    git(root.path(), &["init", "--quiet"]);
    for name in ["kept.txt", "rename.txt", "delete.txt"] {
        std::fs::write(root.path().join(name), "owned").unwrap();
    }
    git(root.path(), &["add", "."]);
    let mut app = loaded(root.path());
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    let mut watch = Runtime::default();
    let started = Instant::now();
    std::fs::write(root.path().join("registered"), "owned").unwrap();
    converge(&mut app, &mut pool, &mut watch, started, "registered");
    app.panes[0].cursor = Some(root.path().join("kept.txt"));
    app.panes[0].toggle_mark();
    let before = pool.listing_metrics();
    let installs = app.panes[0].listing_installs;
    git(root.path(), &["mv", "rename.txt", "renamed.txt"]);
    git(root.path(), &["rm", "--force", "--quiet", "delete.txt"]);
    std::fs::write(root.path().join("created.txt"), "created").unwrap();
    git(root.path(), &["add", "."]);
    until(&mut app, &mut pool, &mut watch, started, |app| {
        app.panes.iter().all(|p| {
            p.entries.iter().any(|e| e.name == "created.txt")
                && p.entries.iter().any(|e| e.name == "renamed.txt")
                && !p
                    .entries
                    .iter()
                    .any(|e| e.name == "rename.txt" || e.name == "delete.txt")
        })
    });
    let after = pool.listing_metrics();
    eprintln!(
        "Git create/rename/delete: full scans {}, targeted {}, installs/pane {}",
        after.full_scans - before.full_scans,
        after.metadata_refreshes - before.metadata_refreshes,
        app.panes[0].listing_installs - installs
    );
    assert_eq!(
        app.panes[0].cursor.as_deref(),
        Some(root.path().join("kept.txt").as_path())
    );
    assert!(
        app.panes[0]
            .marks
            .contains_key(&root.path().join("kept.txt"))
    );
    assert_manual_listing_matches(&app, root.path());
}
