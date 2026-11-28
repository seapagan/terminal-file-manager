use super::*;

#[tokio::test]
async fn activation_stats_targets_and_only_lists_directories() {
    let (peer, mut events) = peer_with_sftp().await;
    let ssh = std::sync::Arc::new(peer.connect(Credentials::Password(PASSWORD)).await.unwrap());
    let worker_session = ssh.clone();
    tokio::task::spawn_blocking(move || {
        check_targets(worker_session, &mut events);
    })
    .await
    .unwrap();
    std::sync::Arc::into_inner(ssh)
        .unwrap()
        .disconnect()
        .await
        .unwrap();
    peer.server_finished().await.unwrap();
}

fn check_targets(
    ssh: std::sync::Arc<terminal_file_manager::ssh::Session>,
    events: &mut UnboundedReceiver<Event>,
) {
    use terminal_file_manager::{
        app::{Request, RequestKind},
        fs::IgnoreContext,
        workers::{Pool, Value},
    };
    let mut pool = Pool::new(IgnoreContext::isolated()).unwrap();
    pool.attach_sftp(ssh);
    let runtime = tokio::runtime::Handle::current();
    let paths: [(&[u8], &str); 5] = [
        (b"/directory-link-\xff", ""),
        (b"/file-link", ""),
        (b"/broken-link", "No such file"),
        (b"/inaccessible-link", "Permission denied"),
        (b"/unknown", "target type is unavailable"),
    ];
    for (i, (path, error)) in paths.into_iter().enumerate() {
        pool.submit(Request {
            pane: 1,
            generation: i as u64,
            path: None,
            kind: RequestKind::SftpActivate(path.into()),
        });
        runtime.block_on(expect_event(events, Event::TargetStat(path.into())));
        let completed = pool.next(LIMIT).unwrap();
        match (path, completed.value) {
            (b"/directory-link-\xff", Value::SftpListing(Ok(entries))) => {
                assert_eq!(
                    entries[0].remote_path(),
                    Some(b"/directory-link-\xff/entry-\xff".as_slice())
                );
                runtime.block_on(expect_event(events, Event::HandleClosed));
            }
            (b"/file-link", Value::SftpFile) => {}
            (_, Value::SftpListing(Err(message))) if !error.is_empty() => assert!(
                message.contains(error) && !message.starts_with("SFTP directory listing"),
                "{message}"
            ),
            _ => panic!("unexpected completion for {path:?}"),
        }
        runtime.block_on(expect_event(events, Event::SubsystemEnded));
    }
}
