use super::*;

#[tokio::test]
async fn delayed_sftp_listing_leaves_native_listing_lane_available() {
    let (events, rx) = mpsc::unbounded_channel();
    let (release, list_gate) = oneshot::channel();
    let peer = Peer::start_with(Subsystem {
        events: Some(events),
        list_gates: [list_gate].into(),
        ..Subsystem::default()
    })
    .await;
    let ssh = std::sync::Arc::new(peer.connect(Credentials::Password(PASSWORD)).await.unwrap());
    let worker_session = ssh.clone();
    tokio::task::spawn_blocking(move || drive_listing_lanes(worker_session, rx, release))
        .await
        .unwrap();
    std::sync::Arc::into_inner(ssh)
        .unwrap()
        .disconnect()
        .await
        .unwrap();
    peer.server_finished().await.unwrap();
}

fn drive_listing_lanes(
    ssh: std::sync::Arc<terminal_file_manager::ssh::Session>,
    mut events: UnboundedReceiver<Event>,
    release: oneshot::Sender<()>,
) {
    use terminal_file_manager::{
        app::{Request, RequestKind},
        fs::IgnoreContext,
        workers::{Pool, Value},
    };
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    pool.attach_sftp(ssh);
    pool.submit(Request {
        pane: 1,
        generation: 1,
        path: None,
        kind: RequestKind::SftpListing(HOME.into()),
    });
    tokio::runtime::Handle::current().block_on(expect_event(&mut events, Event::ListingBlocked));
    let local = tempfile::tempdir().unwrap();
    pool.submit(Request {
        pane: 0,
        generation: 1,
        path: Some(local.path().into()),
        kind: RequestKind::Listing,
    });
    let completed = pool.next(LIMIT).unwrap();
    assert_eq!(completed.request.pane, 0);
    assert!(matches!(completed.value, Value::Listing(Ok(listing)) if listing.path == local.path()));

    release.send(()).unwrap();
    let completed = pool.next(LIMIT).unwrap();
    assert_eq!(completed.request.pane, 1);
    let Value::SftpListing(Ok(entries)) = completed.value else {
        panic!("expected a successful SFTP listing");
    };
    assert_eq!(entries[0].remote_name(), Some(b"entry-\xff".as_slice()));
    assert!(entries.iter().all(|entry| entry.native_path().is_none()));
}

#[tokio::test]
async fn superseded_sftp_listings_close_both_channels_and_release_the_latest_request() {
    let (events, rx) = mpsc::unbounded_channel();
    let (listing_closes, mut closes) = mpsc::unbounded_channel();
    let (release_first, first) = oneshot::channel();
    let (release_second, second) = oneshot::channel();
    let peer = Peer::start_with(Subsystem {
        events: Some(events),
        list_gates: [first, second].into(),
        listing_closes: Some(listing_closes),
        ..Subsystem::default()
    })
    .await;
    let ssh = std::sync::Arc::new(peer.connect(Credentials::Password(PASSWORD)).await.unwrap());
    let worker_session = ssh.clone();
    let mut events = tokio::task::spawn_blocking(move || supersede_listings(worker_session, rx))
        .await
        .unwrap();
    let first = timeout(LIMIT, closes.recv()).await.unwrap().unwrap();
    let second = timeout(LIMIT, closes.recv()).await.unwrap().unwrap();
    assert_ne!(first, second);
    // Keep both requests stalled until the server has observed both SSH CLOSE messages.
    drop((release_first, release_second));
    for _ in 0..2 {
        expect_event(&mut events, Event::SubsystemEnded).await;
    }
    std::sync::Arc::into_inner(ssh)
        .unwrap()
        .disconnect()
        .await
        .unwrap();
    peer.server_finished().await.unwrap();
}

fn supersede_listings(
    ssh: std::sync::Arc<terminal_file_manager::ssh::Session>,
    mut events: UnboundedReceiver<Event>,
) -> UnboundedReceiver<Event> {
    use terminal_file_manager::{
        app::{Request, RequestKind},
        fs::IgnoreContext,
        workers::{Pool, Value},
    };

    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    pool.attach_sftp(ssh);
    let runtime = tokio::runtime::Handle::current();
    let request = |generation, path: &[u8]| Request {
        pane: 1,
        generation,
        path: None,
        kind: RequestKind::SftpListing(path.into()),
    };
    for generation in 1..=2 {
        pool.submit(request(generation, b"/empty-\xfe"));
        runtime.block_on(expect_event(&mut events, Event::ListingBlocked));
    }
    pool.submit(request(3, HOME));
    assert_eq!(pool.active_count(), 2);
    assert_eq!(pool.pending_count(), 1);

    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut finished = (0..3)
        .map(|_| {
            pool.next(deadline.saturating_duration_since(std::time::Instant::now()))
                .unwrap()
        })
        .collect::<Vec<_>>();
    finished.sort_by_key(|item| item.request.generation);
    assert!(matches!(finished[0].value, Value::Cancelled));
    assert!(matches!(finished[1].value, Value::Cancelled));
    let latest = finished.pop().unwrap();
    assert_eq!(latest.request.generation, 3);
    let Value::SftpListing(Ok(entries)) = latest.value else {
        panic!("expected latest SFTP listing to succeed");
    };
    assert_eq!(entries[0].remote_name(), Some(b"entry-\xff".as_slice()));
    runtime.block_on(expect_event(&mut events, Event::HandleClosed));
    runtime.block_on(expect_event(&mut events, Event::SubsystemEnded));
    events
}
