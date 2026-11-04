use super::*;

#[test]
fn elapsed_time_keeps_a_fixed_right_aligned_column_and_two_cell_gap() {
    let mut c = controller();
    let created = SystemTime::UNIX_EPOCH;
    c.app.now = created;
    c.app.notify(Severity::Error, "界".repeat(80));
    c.app.notifications.open = true;
    let mut first = None;
    for (seconds, expected) in [
        (0, "now"),
        (9, "9s"),
        (59, "59s"),
        (60, "1m"),
        (3600, "1h"),
        (8640000, "100d"),
    ] {
        c.app.now = created + Duration::from_secs(seconds);
        let (buffer, geometry) = draw(&c, 80);
        let body = geometry.notification_history.unwrap().shell.body;
        let stamp = ratatui::layout::Rect::new(body.right() - 8, body.y, 8, 1);
        assert_eq!(text(&buffer, stamp), format!("{expected:>8}"));
        let gap = ratatui::layout::Rect::new(body.right() - 10, body.y, 2, 1);
        assert_eq!(text(&buffer, gap), "  ");
        let summary = text(
            &buffer,
            ratatui::layout::Rect::new(body.x, body.y, body.width - 10, 1),
        );
        if let Some(first) = &first {
            assert_eq!(&summary, first);
        } else {
            first = Some(summary);
        }
    }
}

#[test]
fn directory_self_targets_and_descendants_have_distinct_operation_specific_copy() {
    use terminal_file_manager::operations::{Operation, Plan};
    for (operation, verb) in [(Operation::Copy, "copy"), (Operation::Move, "move")] {
        for descendant in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let folder = root.path().join("folder");
            std::fs::create_dir_all(folder.join("child")).unwrap();
            std::fs::write(folder.join("keep"), "preserved").unwrap();
            let destination = if descendant {
                folder.join("child")
            } else {
                root.path().into()
            };
            let mut c = controller();
            complete(
                &mut c,
                Plan::paths(operation, vec![folder.clone()], Some(destination)).unwrap(),
            );
            let notice = c.app.notifications.history.last().unwrap();
            let ending = if descendant {
                "into one of its descendants"
            } else {
                "onto itself"
            };
            assert_eq!(
                notice.presentation.message,
                format!("Cannot {verb} directory “folder” {ending}.")
            );
            let detail = if descendant {
                "Destination is inside the source folder."
            } else {
                "Source and destination are the same folder."
            };
            assert!(notice.text.contains(detail));
            assert_eq!(
                c.app.operations.history[0].record.as_ref().unwrap().entries[0].detail,
                detail
            );
            assert_eq!(std::fs::read(folder.join("keep")).unwrap(), b"preserved");
            assert!(!folder.join("child/folder").exists());
        }
    }
}
