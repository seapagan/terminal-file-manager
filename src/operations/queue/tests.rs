use super::*;

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
