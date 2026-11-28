use std::io;

use russh::{Channel, ChannelMsg, client};
use russh_sftp::{
    client::{SftpSession, error::Error, fs::ReadDir},
    protocol::{FileType, SftpString, Status, StatusCode},
};
use tokio::io::{DuplexStream, copy_bidirectional, duplex};

use super::{CONNECT_TIMEOUT, DISCONNECT_TIMEOUT, Session, timed_out, timeout};

const SFTP_STREAM_BUFFER: usize = 64 * 1024;

enum Target<'a> {
    List(&'a [u8]),
    Resolve(&'a [u8]),
    Activate(&'a [u8]),
}

type Directory = (SftpString, ReadDir);

fn require_directory(result: Option<Directory>) -> io::Result<Directory> {
    result.ok_or_else(|| io::Error::other("Expected an SFTP directory listing"))
}

impl Session {
    /// List one directory over a dedicated SFTP channel.
    ///
    /// Pass the exact remote path bytes or `None` to resolve the server's
    /// initial directory with REALPATH ".". The returned path and entry names
    /// preserve the bytes; the entry attributes retain their optional fields.
    ///
    /// # Errors
    /// Returns an error if the server refuses SFTP, the listing fails, or
    /// negotiation, listing or channel shutdown exceeds its deadline.
    pub async fn list_sftp(&self, path: Option<&[u8]>) -> io::Result<(SftpString, ReadDir)> {
        let target = path.map_or(Target::Resolve(b"."), Target::List);
        require_directory(self.list_sftp_path(target).await?)
    }

    /// List the initial browser location, resolving explicit relative paths on the server.
    ///
    /// # Errors
    /// Returns an error if path resolution, listing, or channel shutdown fails.
    pub async fn list_sftp_start(&self, path: Option<&[u8]>) -> io::Result<(SftpString, ReadDir)> {
        require_directory(
            self.list_sftp_path(Target::Resolve(path.unwrap_or(b".")))
                .await?,
        )
    }

    pub(crate) async fn activate_sftp(&self, path: &[u8]) -> io::Result<Option<ReadDir>> {
        self.list_sftp_path(Target::Activate(path))
            .await
            .map(|listing| listing.map(|(_, entries)| entries))
    }

    async fn list_sftp_path(&self, target: Target<'_>) -> io::Result<Option<Directory>> {
        let mut channel = open_channel(self).await?.into_stream();
        let (stream, mut bridge) = duplex(SFTP_STREAM_BUFFER);
        // The SFTP library's close only queues shutdown. Keep the SSH stream
        // owned here so a stalled bridge can be dropped, closing the channel.
        let forward = async move {
            copy_bidirectional(&mut bridge, &mut channel)
                .await
                .map(|_| ())
                .map_err(|error| {
                    io::Error::new(error.kind(), format!("SFTP channel I/O failed: {error}"))
                })
        };
        let listing = list_over_stream(stream, target);
        tokio::pin!(forward, listing);
        let (listing, closed) = tokio::select! {
            result = &mut listing => {
                let closed = timeout(DISCONNECT_TIMEOUT, &mut forward)
                    .await
                    .unwrap_or_else(|_| Err(timed_out("SFTP channel closure timed out")));
                (result, closed)
            }
            closed = &mut forward => (listing.await, closed),
        };
        listing.and_then(|listing| closed.map(|()| listing))
    }
}

async fn list_over_stream(
    stream: DuplexStream,
    target: Target<'_>,
) -> io::Result<Option<Directory>> {
    let sftp = timeout(CONNECT_TIMEOUT, SftpSession::new(stream))
        .await
        .map_err(|_| timed_out("SFTP initialization timed out"))?
        .map_err(|error| sftp_error("SFTP initialization", error))?;
    let listing = timeout(CONNECT_TIMEOUT, list_directory(&sftp, target))
        .await
        .unwrap_or_else(|_| Err(timed_out("SFTP directory listing timed out")));
    let closed = sftp
        .close()
        .await
        .map_err(|error| sftp_error("SFTP close", error));
    listing.and_then(|listing| closed.map(|()| listing))
}

async fn open_channel(ssh: &Session) -> io::Result<Channel<client::Msg>> {
    let mut channel = timeout(CONNECT_TIMEOUT, ssh.0.channel_open_session())
        .await
        .map_err(|_| timed_out("Opening the SSH channel for SFTP timed out"))?
        .map_err(|error| {
            io::Error::other(format!("Could not open SSH channel for SFTP: {error}"))
        })?;
    let requested = timeout(CONNECT_TIMEOUT, request_sftp(&mut channel))
        .await
        .unwrap_or_else(|_| Err(timed_out("SFTP subsystem request timed out")));
    if let Err(error) = requested {
        let _ = timeout(DISCONNECT_TIMEOUT, channel.close()).await;
        return Err(error);
    }
    Ok(channel)
}

async fn request_sftp(channel: &mut Channel<client::Msg>) -> io::Result<()> {
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|error| io::Error::other(format!("Could not request SFTP: {error}")))?;
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Success => return Ok(()),
            ChannelMsg::Failure => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "SSH server refused the SFTP subsystem",
                ));
            }
            ChannelMsg::Close | ChannelMsg::Eof => break,
            _ => {}
        }
    }
    Err(io::Error::new(
        io::ErrorKind::BrokenPipe,
        "SSH channel closed before the SFTP subsystem started",
    ))
}

async fn list_directory(sftp: &SftpSession, target: Target<'_>) -> io::Result<Option<Directory>> {
    let path = match target {
        Target::List(path) => SftpString::from(path),
        Target::Resolve(path) => sftp
            .canonicalize(SftpString::from(path))
            .await
            .map_err(|error| sftp_error("Resolving the initial SFTP directory", error))?,
        Target::Activate(raw) => {
            let path = SftpString::from(raw);
            if !directory_target(sftp, &path).await? {
                return Ok(None);
            }
            path
        }
    };
    let entries = sftp
        .read_dir(path.clone())
        .await
        .map_err(|error| sftp_error("SFTP directory listing", error))?;
    Ok(Some((path, entries)))
}

async fn directory_target(sftp: &SftpSession, path: &SftpString) -> io::Result<bool> {
    let attributes = sftp
        .metadata(path.clone())
        .await
        .map_err(|error| sftp_error("Resolving the SFTP target", error))?;
    match attributes.file_type() {
        FileType::Dir => Ok(true),
        FileType::File => Ok(false),
        _ => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "SFTP target type is unavailable or unsupported",
        )),
    }
}

fn sftp_error(operation: &str, error: Error) -> io::Error {
    let kind = match &error {
        Error::Timeout => io::ErrorKind::TimedOut,
        Error::Status(status) => match status.status_code {
            StatusCode::PermissionDenied => io::ErrorKind::PermissionDenied,
            StatusCode::NoSuchFile => io::ErrorKind::NotFound,
            StatusCode::NoConnection | StatusCode::ConnectionLost => io::ErrorKind::BrokenPipe,
            StatusCode::OpUnsupported => io::ErrorKind::Unsupported,
            _ => io::ErrorKind::Other,
        },
        _ => io::ErrorKind::Other,
    };
    let message = match &error {
        Error::Status(status) => status_message(status),
        _ => error.to_string(),
    };
    io::Error::new(kind, format!("{operation} failed: {message}"))
}

fn status_message(status: &Status) -> String {
    let code = status.status_code.to_string();
    let detail = status.error_message.trim();
    let detail = match detail.split_once(':') {
        Some((prefix, rest)) if prefix.trim().eq_ignore_ascii_case(&code) => rest.trim(),
        _ => detail,
    };
    if detail.is_empty() || detail.eq_ignore_ascii_case(&code) {
        code
    } else {
        format!("{code}: {detail}")
    }
}
