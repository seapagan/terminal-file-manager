use super::*;

#[test]
fn collision_toast_names_the_file_and_history_routes_to_retained_jobs_evidence() {
    use terminal_file_manager::{color::Theme, notifications::Severity};
    let (root, mut c) = loaded();
    fs::write(root.path().join("collision"), "preserved").unwrap();
    let plan = submit(&mut c, "collision");
    finish(&mut c, &plan);
    let notice = c.app.notifications.history.last().unwrap();
    assert_eq!(notice.severity, Severity::Error);
    assert!(notice.text.starts_with("collision:"));
    assert!(
        notice.text.to_lowercase().contains("exist"),
        "{}",
        notice.text
    );
    assert_eq!(notice.job, Some(1));
    let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
    t.draw(|f| {
        ui::render(f, &c.app, &Theme::new(colored_text::ColorLevel::NoColor));
    })
    .unwrap();
    let text: String = t
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Create file failed"), "{text}");
    assert!(text.contains("“collision” already exists."), "{text}");
    c.app.notifications.advance(Duration::from_secs(60));
    key(&mut c, "Alt+h");
    assert!(c.app.notifications.open);
    key(&mut c, "Enter");
    assert!(c.app.operations.show_history && c.app.operations.jobs_evidence);
    assert_eq!(c.app.operations.cursor, 0);
    assert_eq!(
        fs::read(root.path().join("collision")).unwrap(),
        b"preserved"
    );
    assert_eq!(c.app.operations.history.len(), 1);
    assert_eq!(c.app.notifications.history.len(), 1);
}

#[test]
fn repeated_collisions_group_toasts_without_merging_jobs_or_their_evidence() {
    let (root, mut c) = loaded();
    fs::write(root.path().join("collision"), "preserved").unwrap();
    for _ in 0..3 {
        let plan = submit(&mut c, "collision");
        finish(&mut c, &plan);
    }
    let notices = &c.app.notifications;
    assert_eq!(notices.visible(), [2]);
    assert_eq!(notices.toast_occurrences(2), 3);
    assert_eq!(notices.history.len(), 3);
    for (index, event) in notices.history.iter().enumerate() {
        let job = &c.app.operations.history[index];
        assert_eq!(event.job, Some(job.id));
        assert!(job.can_retry());
        let entry = &job.record.as_ref().unwrap().entries[0];
        assert_eq!(
            entry.destination.as_deref(),
            Some(root.path().join("collision").as_path())
        );
        assert_eq!(entry.outcome, Outcome::Failed);
        assert!(event.text.contains(&entry.detail));
    }
    for index in 0..3 {
        c.app.notifications.open = true;
        c.app.notifications.cursor = index;
        key(&mut c, "Enter");
        assert_eq!(c.app.operations.cursor, index);
        assert!(c.app.operations.jobs_evidence);
    }
    assert_eq!(
        fs::read(root.path().join("collision")).unwrap(),
        b"preserved"
    );
}
