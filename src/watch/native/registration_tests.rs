use super::*;

#[test]
fn parent_disappearing_after_current_watch_rejects_partial_registration() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("parent");
    let path = parent.join("current");
    let moved = root.path().join("moved");
    std::fs::create_dir_all(&path).unwrap();
    let mut watcher = notify::recommended_watcher(|_| {}).unwrap();
    let mut calls = Vec::new();
    let result = watch_paths(&path, |target| {
        calls.push(target.to_path_buf());
        watcher.watch(target, RecursiveMode::NonRecursive)?;
        if target == path {
            // The real current-directory registration succeeds before parent failure.
            std::fs::rename(&parent, &moved).unwrap();
        }
        Ok(())
    });
    assert_eq!(calls, [path, parent]);
    assert!(result.is_err(), "partial watcher must never be healthy");
    assert!(moved.join("current").is_dir());
}
