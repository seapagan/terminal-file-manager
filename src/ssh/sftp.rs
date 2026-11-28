use std::io;

use russh::{Channel, ChannelMsg, client};
use russh_sftp::{
    client::{SftpSession, error::Error, fs::ReadDir},
    protocol::{SftpString, Status, StatusCode},
};
use tokio::io::{DuplexStream, copy_bidirectional, duplex};

use super::{CONNECT_TIMEOUT, DISCONNECT_TIMEOUT, Session, timed_out, timeout};

const SFTP_STREAM_BUFFER: usize = 64 * 1024;

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
        let listing = list_over_stream(stream, path);
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
    path: Option<&[u8]>,
) -> io::Result<(SftpString, ReadDir)> {
    let sftp = timeout(CONNECT_TIMEOUT, SftpSession::new(stream))
        .await
        .map_err(|_| timed_out("SFTP initialization timed out"))?
        .map_err(|error| sftp_error("SFTP initialization", error))?;
    let listing = timeout(CONNECT_TIMEOUT, list_directory(&sftp, path))
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

async fn list_directory(
    sftp: &SftpSession,
    path: Option<&[u8]>,
) -> io::Result<(SftpString, ReadDir)> {
    let path = match path {
        Some(path) => SftpString::from(path),
        None => sftp
            .canonicalize(".")
            .await
            .map_err(|error| sftp_error("Resolving the initial SFTP directory", error))?,
    };
    let entries = sftp
        .read_dir(path.clone())
        .await
        .map_err(|error| sftp_error("SFTP directory listing", error))?;
    Ok((path, entries))
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
