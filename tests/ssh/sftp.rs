use std::{io::ErrorKind, time::Duration};

use russh::{Channel, ChannelId, ChannelMsg, Disconnect, server};
use russh_sftp::{
    protocol::{
        Attrs, File, FileAttributes, FileType, Handle, Name, SftpHandle, SftpString, Status,
        StatusCode,
    },
    server as sftp,
};
use tokio::{
    io::{self, AsyncWriteExt, DuplexStream},
    sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
};

use super::{Credentials, LIMIT, PASSWORD, Peer, timeout};

const HOME: &[u8] = b"/home/user-\xff";
const HANDLE: &[u8] = b"\xff-dir";

type RefusedChannel = (Channel<server::Msg>, server::Handle);

#[derive(Debug, PartialEq, Eq)]
enum Event {
    HandleClosed,
    SubsystemEnded,
    StalledChannelClosed,
    ListingBlocked,
    TargetStat(SftpString),
}

#[derive(Default)]
pub(super) struct Subsystem {
    pub channel: Option<Channel<server::Msg>>,
    refused_channel: Option<oneshot::Sender<RefusedChannel>>,
    refused_close_ping: Option<oneshot::Sender<()>>,
    events: Option<UnboundedSender<Event>>,
    stall_close: bool,
    list_gate: Option<oneshot::Receiver<()>>,
    stalled: Option<(ChannelId, oneshot::Sender<()>)>,
}

impl Subsystem {
    pub async fn start(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), russh::Error> {
        let channel = self.channel.take().unwrap();
        if let Some(events) = &self.events
            && name == "sftp"
        {
            session.channel_success(id)?;
            let directory = Directory {
                events: events.clone(),
                files: None,
                ssh: session.handle(),
                list_gate: self.list_gate.take(),
                stated_directory: false,
            };
            if std::mem::take(&mut self.stall_close) {
                let (stream, peer) = io::duplex(8192);
                let (release, closed) = oneshot::channel();
                self.stalled = Some((id, release));
                sftp::run(stream, directory).await;
                tokio::spawn(withhold_close(channel, peer, closed));
            } else {
                sftp::run(channel.into_stream(), directory).await;
            }
        } else {
            session.channel_failure(id)?;
            if let Some(refused) = self.refused_channel.take() {
                assert!(refused.send((channel, session.handle())).is_ok());
            } else {
                session.close(id)?;
            }
        }
        Ok(())
    }

    pub fn channel_closed(
        &mut self,
        id: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), russh::Error> {
        if let Some(reply) = self.refused_close_ping.take() {
            // russh queues its CHANNEL_CLOSE reply before invoking this callback.
            session.send_ping(reply)?;
        }
        if self
            .stalled
            .as_ref()
            .is_some_and(|(stalled, _)| *stalled == id)
        {
            self.stalled.take();
            if let Some(events) = &self.events {
                let _ = events.send(Event::StalledChannelClosed);
            }
        }
        Ok(())
    }
}

async fn withhold_close(
    channel: Channel<server::Msg>,
    sftp: DuplexStream,
    closed: oneshot::Receiver<()>,
) {
    let (mut channel_read, mut channel_write) = io::split(channel.into_stream());
    let (mut sftp_read, mut sftp_write) = io::split(sftp);
    let consume_eof = async {
        io::copy(&mut channel_read, &mut sftp_write).await.unwrap();
        sftp_write.shutdown().await.unwrap();
    };
    let (_, reply) = tokio::join!(consume_eof, io::copy(&mut sftp_read, &mut channel_write));
    reply.unwrap();
    // Keep the SSH channel open after the SFTP handler receives EOF and exits.
    let _ = closed.await;
}

struct Directory {
    events: UnboundedSender<Event>,
    files: Option<Vec<File>>,
    ssh: server::Handle,
    list_gate: Option<oneshot::Receiver<()>>,
    stated_directory: bool,
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = self.events.send(Event::SubsystemEnded);
    }
}

impl sftp::Handler for Directory {
    type Error = sftp::StatusReply;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported.into()
    }

    async fn realpath(&mut self, id: u32, path: SftpString) -> Result<Name, Self::Error> {
        assert_eq!(path.as_bytes(), b".");
        Ok(Name {
            id,
            files: vec![File::dummy(HOME)],
        })
    }

    async fn stat(&mut self, id: u32, path: SftpString) -> Result<Attrs, Self::Error> {
        self.events.send(Event::TargetStat(path.clone())).unwrap();
        let permissions = match path.as_bytes() {
            b"/directory-link-\xff" => {
                self.stated_directory = true;
                Some(0o040755)
            }
            b"/file-link" => Some(0o100644),
            b"/unknown" => None,
            b"/inaccessible-link" => return Err(StatusCode::PermissionDenied.into()),
            _ => return Err(StatusCode::NoSuchFile.into()),
        };
        Ok(Attrs {
            id,
            attrs: FileAttributes {
                permissions,
                ..Default::default()
            },
        })
    }

    async fn opendir(&mut self, id: u32, path: SftpString) -> Result<Handle, Self::Error> {
        if let Some(gate) = self.list_gate.take() {
            self.events.send(Event::ListingBlocked).unwrap();
            gate.await.map_err(|_| StatusCode::Failure)?;
        }
        self.files = Some(match path.as_bytes() {
            HOME => fixture_files(),
            b"/directory-link-\xff" => {
                assert!(
                    self.stated_directory,
                    "STAT and OPENDIR must share a channel"
                );
                fixture_files()
            }
            b"/empty-\xfe" => vec![File::dummy("."), File::dummy("..")],
            b"/denied" => {
                return Err(StatusCode::PermissionDenied.with_message("directory is restricted"));
            }
            b"/prefixed" => {
                return Err(StatusCode::PermissionDenied
                    .with_message("Permission denied: directory is restricted"));
            }
            b"/lost" => {
                self.ssh
                    .disconnect(
                        Disconnect::ByApplication,
                        "fixture closed SSH".into(),
                        String::new(),
                    )
                    .await
                    .unwrap();
                return Err(StatusCode::Failure.into());
            }
            _ => return Err(StatusCode::NoSuchFile.into()),
        });
        Ok(Handle {
            id,
            handle: HANDLE.to_vec().into(),
        })
    }

    async fn readdir(&mut self, id: u32, handle: SftpHandle) -> Result<Name, Self::Error> {
        assert_eq!(handle.as_bytes(), HANDLE);
        let files = self.files.take().ok_or(StatusCode::Eof)?;
        Ok(Name { id, files })
    }

    async fn close(&mut self, id: u32, handle: SftpHandle) -> Result<Status, Self::Error> {
        assert_eq!(handle.as_bytes(), HANDLE);
        self.events.send(Event::HandleClosed).unwrap();
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: String::new(),
            language_tag: String::new(),
        })
    }
}

fn fixture_files() -> Vec<File> {
    let regular = FileAttributes {
        size: Some(5),
        uid: Some(42),
        gid: Some(43),
        permissions: Some(0o100640),
        atime: Some(0),
        mtime: Some(1234),
        ..FileAttributes::default()
    };
    let link = FileAttributes {
        permissions: Some(0o120777),
        ..FileAttributes::default()
    };
    vec![
        File::dummy("."),
        File::new(b"entry-\xff", regular),
        File::new(b"entry-\xfe", link),
        File::new("entry-�", FileAttributes::default()),
        File::dummy(".."),
    ]
}

async fn peer_with_sftp() -> (Peer, UnboundedReceiver<Event>) {
    sftp_peer(false).await
}

async fn sftp_peer(stall_close: bool) -> (Peer, UnboundedReceiver<Event>) {
    let (events, rx) = mpsc::unbounded_channel();
    (
        Peer::start_with(Subsystem {
            events: Some(events),
            stall_close,
            ..Subsystem::default()
        })
        .await,
        rx,
    )
}

async fn expect_event(events: &mut UnboundedReceiver<Event>, event: Event) {
    assert_eq!(
        timeout(LIMIT, events.recv())
            .await
            .expect("SFTP fixture stayed open"),
        Some(event)
    );
}

#[tokio::test]
async fn lists_initial_directory_with_byte_exact_names_and_metadata() {
    let (peer, mut events) = peer_with_sftp().await;
    let ssh = peer.connect(Credentials::Password(PASSWORD)).await.unwrap();
    let (path, entries) = ssh.list_sftp(None).await.unwrap();
    let entries: Vec<_> = entries.collect();
    assert_eq!(path.as_bytes(), HOME);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].path().as_bytes(), b"/home/user-\xff/entry-\xff");
    assert_eq!(entries[0].file_type(), FileType::File);
    let attrs = entries[0].metadata();
    assert_eq!(attrs.size, Some(5));
    assert_eq!((attrs.uid, attrs.gid), (Some(42), Some(43)));
    assert_eq!(attrs.permissions, Some(0o100640));
    assert_eq!((attrs.atime, attrs.mtime), (Some(0), Some(1234)));
    assert_eq!(entries[1].file_name().as_bytes(), b"entry-\xfe");
    assert_eq!(entries[1].file_type(), FileType::Symlink);
    assert_eq!(entries[2].file_name().as_bytes(), "entry-�".as_bytes());
    let absent = entries[2].metadata();
    assert_eq!((absent.size, absent.permissions), (None, None));
    assert_eq!(
        (absent.uid, absent.gid, absent.atime, absent.mtime),
        (None, None, None, None)
    );
    expect_event(&mut events, Event::HandleClosed).await;
    expect_event(&mut events, Event::SubsystemEnded).await;
    ssh.disconnect().await.unwrap();
    peer.server_finished().await.unwrap();
}

#[tokio::test]
async fn denies_inaccessible_paths_and_lists_an_empty_byte_path_on_same_ssh_connection() {
    let (peer, mut events) = peer_with_sftp().await;
    let ssh = peer.connect(Credentials::Password(PASSWORD)).await.unwrap();
    for (path, kind, message) in [
        (
            b"/denied".as_slice(),
            ErrorKind::PermissionDenied,
            "Permission denied: directory is restricted",
        ),
        (
            b"/prefixed",
            ErrorKind::PermissionDenied,
            "Permission denied: directory is restricted",
        ),
        (b"/absent", ErrorKind::NotFound, "No such file"),
    ] {
        let error = ssh.list_sftp(Some(path)).await.err().unwrap();
        assert_eq!(error.kind(), kind);
        assert_eq!(
            error.to_string(),
            format!("SFTP directory listing failed: {message}")
        );
        expect_event(&mut events, Event::SubsystemEnded).await;
    }
    let (path, mut entries) = ssh.list_sftp(Some(b"/empty-\xfe")).await.unwrap();
    assert_eq!(path.as_bytes(), b"/empty-\xfe");
    assert!(entries.next().is_none());
    expect_event(&mut events, Event::HandleClosed).await;
    expect_event(&mut events, Event::SubsystemEnded).await;
    ssh.disconnect().await.unwrap();
    peer.server_finished().await.unwrap();
}

#[tokio::test]
async fn bounds_unresponsive_sftp_closure_and_reuses_ssh() {
    let (peer, mut events) = sftp_peer(true).await;
    let ssh = peer.connect(Credentials::Password(PASSWORD)).await.unwrap();
    let result = timeout(LIMIT, ssh.list_sftp(Some(b"/empty-\xfe")))
        .await
        .unwrap();
    let error = result.err().expect("unresponsive closure must time out");
    assert_eq!(error.kind(), ErrorKind::TimedOut);
    assert!(error.to_string().contains("SFTP channel closure"));
    expect_event(&mut events, Event::HandleClosed).await;
    expect_event(&mut events, Event::SubsystemEnded).await;
    expect_event(&mut events, Event::StalledChannelClosed).await;

    let (_, mut entries) = ssh.list_sftp(Some(b"/empty-\xfe")).await.unwrap();
    assert!(entries.next().is_none());
    expect_event(&mut events, Event::HandleClosed).await;
    expect_event(&mut events, Event::SubsystemEnded).await;
    ssh.disconnect().await.unwrap();
    peer.server_finished().await.unwrap();
}

#[tokio::test]
async fn reports_a_refused_sftp_subsystem() {
    let peer = Peer::start().await;
    let ssh = peer.connect(Credentials::Password(PASSWORD)).await.unwrap();
    let error = ssh.list_sftp(None).await.err().unwrap();
    assert_eq!(error.kind(), ErrorKind::Unsupported);
    assert_eq!(error.to_string(), "SSH server refused the SFTP subsystem");
    ssh.disconnect().await.unwrap();
    peer.server_finished().await.unwrap();
}

async fn peer_with_refused_channel(
    refused_close_ping: Option<oneshot::Sender<()>>,
) -> (Peer, oneshot::Receiver<RefusedChannel>) {
    let (refused, channel) = oneshot::channel();
    let peer = Peer::start_with(Subsystem {
        refused_channel: Some(refused),
        refused_close_ping,
        ..Subsystem::default()
    })
    .await;
    (peer, channel)
}

#[tokio::test]
async fn reports_refusal_promptly_and_closes_a_channel_left_open_by_the_server() {
    let (reply, close_drained) = oneshot::channel();
    let (peer, channel) = peer_with_refused_channel(Some(reply)).await;
    let ssh = peer.connect(Credentials::Password(PASSWORD)).await.unwrap();
    // Finish before the five-second cleanup deadline while the server sends no CLOSE.
    let error = timeout(Duration::from_secs(3), ssh.list_sftp(None))
        .await
        .expect("client waited for the refusing server to close the channel")
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::Unsupported);
    assert_eq!(error.to_string(), "SSH server refused the SFTP subsystem");

    let (mut channel, _) = timeout(LIMIT, channel).await.unwrap().unwrap();
    timeout(LIMIT, async {
        loop {
            let message = channel.wait().await.expect("peer never sent CHANNEL_CLOSE");
            if matches!(message, ChannelMsg::Close) {
                break;
            }
        }
    })
    .await
    .unwrap();
    // The ping reply proves the client read the preceding CLOSE before disconnect.
    timeout(LIMIT, close_drained).await.unwrap().unwrap();
    ssh.disconnect().await.unwrap();
    peer.server_finished().await.unwrap();
}

#[tokio::test]
async fn preserves_the_sftp_refusal_when_the_transport_closes() {
    let (peer, channel) = peer_with_refused_channel(None).await;
    let ssh = peer.connect(Credentials::Password(PASSWORD)).await.unwrap();
    let disconnect = async {
        let (_channel, server) = channel.await.unwrap();
        server
            .disconnect(Disconnect::ByApplication, String::new(), String::new())
            .await
            .unwrap();
    };
    let (listing, ()) = timeout(LIMIT, async {
        tokio::join!(ssh.list_sftp(None), disconnect)
    })
    .await
    .unwrap();
    let error = listing.err().unwrap();
    assert_eq!(error.kind(), ErrorKind::Unsupported);
    assert_eq!(error.to_string(), "SSH server refused the SFTP subsystem");
    peer.server_finished().await.unwrap();
}

#[tokio::test]
async fn rejects_a_listing_when_the_ssh_connection_is_lost() {
    let (peer, _) = peer_with_sftp().await;
    let ssh = peer.connect(Credentials::Password(PASSWORD)).await.unwrap();
    let result = timeout(LIMIT, ssh.list_sftp(Some(b"/lost"))).await.unwrap();
    let error = result.err().unwrap();
    assert!(
        error.to_string().contains("SFTP directory listing")
            && !error.to_string().contains("Failure"),
        "expected closure of the stream without a directory status reply: {error}"
    );
    peer.server_finished().await.unwrap();
}

#[tokio::test]
async fn delayed_sftp_listing_leaves_native_listing_lane_available() {
    let (events, rx) = mpsc::unbounded_channel();
    let (release, list_gate) = oneshot::channel();
    let peer = Peer::start_with(Subsystem {
        events: Some(events),
        list_gate: Some(list_gate),
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

#[path = "sftp/activation.rs"]
mod activation;
