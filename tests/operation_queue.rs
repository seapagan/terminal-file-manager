use std::{fs, time::Duration};
use terminal_file_manager::operations::{
    Decision, Event, Operation, Options, Outcome, Plan, Queue, Update,
};

fn conflict(root: &std::path::Path, name: &str) -> Plan {
    fs::write(root.join(name), "incoming").unwrap();
    fs::create_dir(root.join(format!("target-{name}"))).unwrap();
    fs::write(root.join(format!("target-{name}/{name}")), "previous").unwrap();
    Plan::paths(
        Operation::Copy,
        vec![root.join(name)],
        Some(root.join(format!("target-{name}"))),
    )
    .unwrap()
}

#[test]
fn two_running_jobs_leave_a_third_queued_and_cancel_it_without_mutation() {
    let root = tempfile::tempdir().unwrap();
    let mut queue = Queue::new().unwrap();
    let a = queue
        .submit(conflict(root.path(), "a"), Options::default())
        .unwrap();
    let b = queue
        .submit(conflict(root.path(), "b"), Options::default())
        .unwrap();
    let c = queue
        .submit(conflict(root.path(), "c"), Options::default())
        .unwrap();
    let directory = root.path().join("cancelled-directory");
    let d = queue
        .submit(Plan::mkdir(directory.clone()), Options::default())
        .unwrap();
    let replies = waiting_conflicts(&mut queue, [a, b]);
    queue.cancel(c);
    queue.cancel(d);
    for reply in replies {
        reply.send(Decision::Skip).unwrap();
    }
    let mut finished = 0;
    while finished < 4 {
        if let Update::Finished { id, record } =
            queue.next(Duration::from_secs(5)).expect("completion")
        {
            if id == c || id == d {
                assert!(!record.entries.is_empty());
                assert!(
                    record
                        .entries
                        .iter()
                        .all(|entry| entry.outcome == Outcome::Cancelled)
                );
            }
            finished += 1;
        }
    }
    assert!(!directory.exists());
    assert_eq!(
        fs::read(root.path().join("target-c/c")).unwrap(),
        b"previous"
    );
}

#[test]
fn independent_directory_copies_both_reach_conflict_before_either_is_released() {
    let root = tempfile::tempdir().unwrap();
    let mut queue = Queue::new().unwrap();
    let mut ids = Vec::new();
    for name in ["a", "b"] {
        let source = root.path().join(name);
        let target = root.path().join(format!("target-{name}"));
        fs::create_dir(&source).unwrap();
        fs::write(source.join("file"), "incoming").unwrap();
        fs::create_dir_all(target.join(name)).unwrap();
        fs::write(target.join(name).join("file"), "previous").unwrap();
        let plan = Plan::paths(Operation::Copy, vec![source], Some(target)).unwrap();
        ids.push(queue.submit(plan, Options::default()).unwrap());
    }
    let replies = waiting_conflicts(&mut queue, [ids[0], ids[1]]);
    for reply in replies {
        reply.send(Decision::Skip).unwrap();
    }
    let mut finished = 0;
    while finished < 2 {
        if let Update::Finished { record, .. } = queue.next(Duration::from_secs(5)).unwrap() {
            assert_eq!(record.completed_files(), 0);
            assert!(
                record
                    .entries
                    .iter()
                    .any(|entry| entry.outcome == Outcome::Skipped)
            );
            finished += 1;
        }
    }
    for name in ["a", "b"] {
        assert_eq!(
            fs::read(root.path().join(name).join("file")).unwrap(),
            b"incoming"
        );
        assert_eq!(
            fs::read(root.path().join(format!("target-{name}/{name}/file"))).unwrap(),
            b"previous"
        );
    }
}

#[test]
fn cancelling_a_waiting_conflict_acknowledges_before_shutdown() {
    let root = tempfile::tempdir().unwrap();
    let mut queue = Queue::new().unwrap();
    let id = queue
        .submit(conflict(root.path(), "source"), Options::default())
        .unwrap();
    loop {
        if matches!(
            queue.next(Duration::from_secs(5)).unwrap(),
            Update::Conflict { .. }
        ) {
            break;
        }
    }
    queue.cancel(id);
    loop {
        if let Update::Finished { record, .. } = queue.next(Duration::from_secs(5)).unwrap() {
            assert!(
                record
                    .entries
                    .iter()
                    .any(|entry| entry.outcome == Outcome::Cancelled)
            );
            break;
        }
    }
    drop(queue);
    assert_eq!(fs::read(root.path().join("source")).unwrap(), b"incoming");
    assert_eq!(
        fs::read(root.path().join("target-source/source")).unwrap(),
        b"previous"
    );
}

#[test]
fn dropping_the_queue_cancels_and_joins_actual_workers() {
    let root = tempfile::tempdir().unwrap();
    let mut queue = Queue::new().unwrap();
    queue
        .submit(conflict(root.path(), "source"), Options::default())
        .unwrap();
    loop {
        if matches!(
            queue.next(Duration::from_secs(5)).unwrap(),
            Update::Conflict {
                event: Event::Conflict { .. },
                ..
            }
        ) {
            break;
        }
    }
    drop(queue);
    assert_eq!(fs::read(root.path().join("source")).unwrap(), b"incoming");
    assert_eq!(
        fs::read(root.path().join("target-source/source")).unwrap(),
        b"previous"
    );
    assert_eq!(
        fs::read_dir(root.path().join("target-source"))
            .unwrap()
            .count(),
        1
    );
}

fn waiting_conflicts(
    queue: &mut Queue,
    ids: [u64; 2],
) -> Vec<std::sync::mpsc::SyncSender<Decision>> {
    let mut replies = Vec::new();
    while replies.len() < 2 {
        match queue.next(Duration::from_secs(5)).expect("worker event") {
            Update::Conflict { id, reply, .. } => {
                assert!(ids.contains(&id));
                replies.push(reply);
            }
            Update::Started { id } => assert!(ids.contains(&id)),
            _ => {}
        }
    }
    replies
}
