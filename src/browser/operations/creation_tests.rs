use super::*;

#[test]
fn create_file_jobs_retry_keeps_the_originating_pane_after_switching() {
    let (root, mut controller, mut queue, _) = fixture();
    let path = root.path().join("new");
    install_listing(&mut controller, root.path());
    let request = controller.app.request_listing(1, root.path().into());
    let snapshot = crate::fs::list(
        root.path(),
        &crate::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    controller
        .app
        .complete_listing(1, request.generation, Ok(snapshot));
    fs::write(&path, "collision").unwrap();
    controller.app.operations.outbox.push(Intent::Submit(
        Plan::create_file(path.clone()),
        Options::default(),
    ));
    submit(&mut controller, &mut queue);
    let parent = acknowledge(&mut controller, &mut queue, Decision::Skip);
    assert_eq!(controller.app.operations.history[0].state, JobState::Failed);
    fs::remove_file(&path).unwrap();
    install_listing(&mut controller, root.path());
    let other_cursor = controller.app.panes[1].cursor.clone();
    controller.app.active = 1;
    key(&mut controller, KeyCode::F(9));
    key(&mut controller, KeyCode::Char('r'));
    assert!(
        matches!(controller.app.operations.outbox.as_slice(), [Intent::Retry(id)] if *id == parent)
    );
    submit(&mut controller, &mut queue);
    acknowledge(&mut controller, &mut queue, Decision::Skip);
    install_listing(&mut controller, root.path());
    let generation = controller.app.panes[1].generation;
    let snapshot = crate::fs::list(
        root.path(),
        &crate::fs::IgnoreContext::isolated(),
        &false.into(),
    )
    .unwrap();
    controller.app.complete_listing(1, generation, Ok(snapshot));
    assert_eq!(controller.app.panes[0].cursor, Some(path));
    assert_eq!(controller.app.panes[1].cursor, other_cursor);
    assert_eq!(controller.app.active, 1);
}
