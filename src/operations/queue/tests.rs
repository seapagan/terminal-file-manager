use super::*;

#[test]
fn superseded_preview_completion_and_queued_cancellation_release_all_worker_state() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("selected");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("sentinel"), "retained").unwrap();
    for operation in [
        super::super::Operation::Delete,
        super::super::Operation::Trash,
    ] {
        let plan = Plan::paths(operation, vec![source.clone()], None).unwrap();
        let mut driver = Driver::new();
        let old = driver.queue.preview(plan.clone()).unwrap();
        driver.pump();
        let held = driver
            .messages
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(matches!(&held, Message::Update(Update::Preview { id, .. }) if *id == old));
        driver.queue.cancel(old);
        let replacement = driver.queue.preview(plan.clone()).unwrap();
        driver.pump();
        assert_eq!(driver.pending.len(), 1);
        receive(
            held,
            &mut driver.active,
            &mut driver.coordinator,
            &driver.updates,
        );
        while !matches!(driver.queue.next(Duration::ZERO).unwrap(), Update::Preview { id, .. } if id == old)
        {
        }
        driver.queue.cancel(replacement);
        driver.pump();
        let Some(Update::Finished { id, record }) = driver.queue.next(Duration::ZERO) else {
            panic!("queued replacement cancellation acknowledgement");
        };
        assert_eq!(id, replacement);
        assert!(
            record
                .entries
                .iter()
                .all(|entry| entry.outcome == Outcome::Cancelled)
        );
        assert!(driver.queue.tokens.is_empty());
        assert!(driver.active.is_empty() && driver.pending.is_empty());
        assert!(driver.coordinator.acquire(999, &plan).unwrap());
        assert_eq!(std::fs::read(source.join("sentinel")).unwrap(), b"retained");
    }
}

#[test]
fn reconciliation_without_evidence_keeps_the_original_quarantined_claims() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source"), "preserved").unwrap();
    std::fs::create_dir(root.path().join("target")).unwrap();
    let plan = Plan::paths(
        super::super::Operation::Move,
        vec![root.path().join("source")],
        Some(root.path().join("target")),
    )
    .unwrap();
    let mut coordinator = Coordinator::default();
    assert!(coordinator.acquire(1, &plan).unwrap());
    coordinator.quarantine(1);
    let mut pending = VecDeque::from([Submission {
        id: 1,
        plan: plan.clone(),
        options: Options::default(),
        cancel: Arc::new(false.into()),
        preview: false,
        reconciliation: Some(unsuccessful(
            &plan,
            Outcome::OutcomeUnknown,
            "lost response",
        )),
    }]);
    let mut active = BTreeMap::new();
    let (events, received) = mpsc::sync_channel(4);
    let (updates, _) = mpsc::sync_channel(4);
    pump(
        &mut pending,
        &mut active,
        &mut coordinator,
        &events,
        &updates,
        false,
    );
    let Message::Update(update) = received.recv_timeout(Duration::from_secs(5)).unwrap() else {
        panic!("expected completion");
    };
    assert!(matches!(&update, Update::Finished { record, .. } if record.unresolved()));
    acknowledge(&update, &mut active, &mut coordinator);
    assert_eq!(coordinator.active(), 0);
    assert!(!coordinator.acquire(2, &plan).unwrap());
    assert_eq!(
        std::fs::read(root.path().join("source")).unwrap(),
        b"preserved"
    );
}

// Drive the production scheduler at its claim handshake, before replying to the worker.
pub(crate) struct Driver {
    pub(crate) queue: Queue,
    submissions: mpsc::Receiver<Submission>,
    updates: mpsc::SyncSender<Update>,
    events: mpsc::SyncSender<Message>,
    messages: mpsc::Receiver<Message>,
    pending: VecDeque<Submission>,
    active: BTreeMap<u64, Running>,
    coordinator: Coordinator,
}

impl Driver {
    pub(crate) fn new() -> Self {
        let (sender, submissions) = mpsc::sync_channel(64);
        let (updates, receiver) = mpsc::sync_channel(64);
        let (events, messages) = mpsc::sync_channel(8);
        Self {
            queue: Queue {
                sender,
                receiver,
                stop: Arc::new(false.into()),
                tokens: HashMap::new(),
                handle: None,
                next_id: 1,
            },
            submissions,
            updates,
            events,
            messages,
            pending: VecDeque::new(),
            active: BTreeMap::new(),
            coordinator: Coordinator::default(),
        }
    }

    pub(crate) fn pump(&mut self) {
        self.pending.extend(self.submissions.try_iter());
        pump(
            &mut self.pending,
            &mut self.active,
            &mut self.coordinator,
            &self.events,
            &self.updates,
            false,
        );
    }

    pub(crate) fn finish(&mut self, expected: u64) -> Record {
        let message = self.messages.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            matches!(&message, Message::Update(Update::Finished { id, .. }) if *id == expected)
        );
        receive(
            message,
            &mut self.active,
            &mut self.coordinator,
            &self.updates,
        );
        loop {
            match self.queue.next(Duration::ZERO).unwrap() {
                Update::Started { id } => assert_eq!(id, expected),
                Update::Finished { id, record } => {
                    assert_eq!(id, expected);
                    assert!(!self.queue.tokens.contains_key(&id));
                    assert!(!self.active.contains_key(&id));
                    return record;
                }
                _ => panic!("unexpected lifecycle update"),
            }
        }
    }

    pub(crate) fn replace_claimed_payload(&mut self, held: &std::path::Path) -> std::path::PathBuf {
        let Message::Claim {
            id,
            root,
            payload,
            accepted,
        } = self.messages.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("worker must claim its stage before writing");
        };
        let path = payload.path.clone();
        let (permit, answer) = mpsc::sync_channel(1);
        receive(
            Message::Claim {
                id,
                root,
                payload,
                accepted: permit,
            },
            &mut self.active,
            &mut self.coordinator,
            &self.updates,
        );
        assert!(answer.recv().unwrap());
        std::fs::rename(&path, held).unwrap();
        std::fs::write(&path, "foreign payload").unwrap();
        accepted.send(true).unwrap();
        path
    }
}

fn recovery_fixture() -> (tempfile::TempDir, Plan) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    std::fs::write(&source, "original bytes").unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&source)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(2_000_000_000))
        .unwrap();
    std::fs::create_dir(&target).unwrap();
    let plan = Plan::paths(
        super::super::Operation::Move,
        vec![source.clone()],
        Some(target.clone()),
    )
    .unwrap();
    (root, plan)
}

fn assert_quarantined(driver: &mut Driver) {
    driver.pump();
    assert_eq!(driver.coordinator.active(), 0);
    assert_eq!(driver.pending.len(), 1);
    assert!(driver.queue.next(Duration::ZERO).is_none());
}

#[test]
fn queue_owned_reconciliation_releases_quarantine_only_after_owned_cleanup() {
    let (root, plan) = recovery_fixture();
    let source = root.path().join("source");
    let target = root.path().join("target");
    let mut driver = Driver::new();
    let id = driver
        .queue
        .submit(plan.clone(), Options::default())
        .unwrap();
    driver.pump();
    let held = root.path().join("held-owned");
    let payload = driver.replace_claimed_payload(&held);
    let record = driver.finish(id);
    assert!(record.reconcilable());
    assert_eq!(
        record.entries[0].recovery.as_ref().unwrap().staging_path(),
        payload.parent()
    );
    let blocked = driver
        .queue
        .submit(Plan::mkdir(target.join("later")), Options::default())
        .unwrap();
    assert_quarantined(&mut driver);
    driver.queue.reconcile(id, plan.clone(), record).unwrap();
    driver.pump();
    let record = driver.finish(id);
    assert!(record.unresolved());
    assert_quarantined(&mut driver);
    assert!(!target.join("later").exists());
    assert_eq!(std::fs::read(&payload).unwrap(), b"foreign payload");
    assert!(!target.join("source").exists());
    std::fs::rename(&payload, root.path().join("foreign-preserved")).unwrap();
    std::fs::rename(&held, &payload).unwrap();
    driver.queue.reconcile(id, plan, record).unwrap();
    driver.pump();
    let resolved = driver.finish(id);
    assert_eq!(resolved.entries[0].outcome, Outcome::Failed, "{resolved:?}");
    assert!(!resolved.unresolved());
    assert!(!payload.parent().unwrap().exists());
    assert!(!target.join("source").exists());
    assert_eq!(std::fs::read(&source).unwrap(), b"original bytes");
    assert_eq!(
        std::fs::read(root.path().join("foreign-preserved")).unwrap(),
        b"foreign payload"
    );
    driver.pump();
    assert_eq!(driver.finish(blocked).completed_directories(), 1);
    assert!(target.join("later").is_dir());
    assert!(driver.pending.is_empty());
}
