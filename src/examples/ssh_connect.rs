use std::{
    env,
    ffi::{OsStr, OsString},
    io::{self, IsTerminal, Write},
    path::Path,
    process::ExitCode,
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use russh::{CryptoVec, keys::load_public_key};
use russh_sftp::{
    client::fs::DirEntry,
    protocol::{FileMode, FileType, SftpString},
};
use terminal_file_manager::{
    ssh::{Credentials, Session},
    ui,
};

const USAGE: &str = "cargo run --locked --example ssh_connect -- HOST PORT USER TRUSTED_HOST_KEY \
    (key PRIVATE_KEY | encrypted-key PRIVATE_KEY | agent | password) [--list [REMOTE_PATH]]";

enum Listing {
    Off,
    Initial,
    Path(SftpString),
}

#[tokio::main]
async fn main() -> ExitCode {
    let result = match connect().await {
        Ok((session, listing)) => run(session, listing).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(()) => {
            println!("Disconnected.");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", ui::clean(&error.to_string()));
            ExitCode::FAILURE
        }
    }
}

async fn connect() -> io::Result<(Session, Listing)> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let [host, port, user, trusted_key, method, extra @ ..] = args.as_slice() else {
        return Err(usage());
    };
    let (host, port, user) = endpoint(host, port, user).ok_or_else(usage)?;
    let auth_count = usize::from(matches!(method.to_str(), Some("key" | "encrypted-key")));
    let (extra, listing) = extra.split_at_checked(auth_count).ok_or_else(usage)?;
    let listing = listing_args(listing)?;
    let trusted_key = load_public_key(trusted_key).map_err(|e| {
        io::Error::other(format!("Could not load trusted SSH host public key: {e}"))
    })?;
    let mut secret = CryptoVec::new();
    let credentials = credentials(method, extra, &mut secret)?;
    let session = Session::connect(host, port, user, trusted_key, credentials).await?;
    Ok((session, listing))
}

fn listing_args(args: &[OsString]) -> io::Result<Listing> {
    match args {
        [] => Ok(Listing::Off),
        [flag] if flag == "--list" => Ok(Listing::Initial),
        [flag, path] if flag == "--list" => Ok(Listing::Path(remote_path(path)?)),
        _ => Err(usage()),
    }
}

fn remote_path(path: &OsStr) -> io::Result<SftpString> {
    #[cfg(unix)]
    let bytes = std::os::unix::ffi::OsStrExt::as_bytes(path);
    #[cfg(windows)]
    let bytes = path
        .to_str()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Remote path is not valid Unicode",
            )
        })?
        .as_bytes();
    if bytes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Remote path is empty",
        ));
    }
    Ok(SftpString::from(bytes))
}

async fn run(session: Session, listing: Listing) -> io::Result<()> {
    let listed = match listing {
        Listing::Off => Ok(()),
        Listing::Initial => show_listing(&session, None).await,
        Listing::Path(path) => show_listing(&session, Some(path.as_bytes())).await,
    };
    let result = listed.and_then(|()| {
        println!("Authenticated. Press Enter to disconnect.");
        io::stdin().read_line(&mut String::new()).map(|_| ())
    });
    result.and(session.disconnect().await)
}

async fn show_listing(session: &Session, path: Option<&[u8]>) -> io::Result<()> {
    let (path, mut entries) = session.list_sftp(path).await?;
    println!("SFTP directory {}", ui::clean(&format!("{path:?}")));
    let Some(first) = entries.next() else {
        println!("(empty)");
        return Ok(());
    };
    for entry in std::iter::once(first).chain(entries) {
        print_entry(&entry);
    }
    Ok(())
}

fn print_entry(entry: &DirEntry) {
    let attrs = entry.metadata();
    let mode = attrs
        .permissions
        .map(|mode| format!("{:04o}", mode & 0o7777));
    println!(
        "{}  {}  size={} mode={} uid={} gid={} atime={} mtime={}",
        ui::clean(&format!("{:?}", entry.file_name())),
        entry_kind(attrs.permissions),
        available(attrs.size),
        available(mode),
        available(attrs.uid),
        available(attrs.gid),
        available(attrs.atime),
        available(attrs.mtime),
    );
}

fn entry_kind(mode: Option<u32>) -> &'static str {
    match mode
        .filter(|mode| !FileMode::from_bits_truncate(*mode).is_empty())
        .map(FileType::from)
    {
        Some(FileType::Dir) => "directory",
        Some(FileType::File) => "file",
        Some(FileType::Symlink) => "symlink",
        Some(FileType::Other) => "other",
        None => "unknown",
    }
}

fn available<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "-".into(), |value| value.to_string())
}

fn credentials<'a>(
    method: &OsStr,
    extra: &'a [OsString],
    secret: &'a mut CryptoVec,
) -> io::Result<Credentials<'a>> {
    Ok(match (method.to_str(), extra) {
        (Some("key"), [path]) => Credentials::PrivateKey {
            path: Path::new(path),
            passphrase: None,
        },
        (Some("encrypted-key"), [path]) => {
            *secret = read_secret("Private key passphrase: ")?;
            Credentials::PrivateKey {
                path: Path::new(path),
                passphrase: Some(secret_text(secret)?),
            }
        }
        (Some("agent"), []) => Credentials::Agent,
        (Some("password"), []) => {
            *secret = read_secret("SSH password: ")?;
            Credentials::Password(secret_text(secret)?)
        }
        _ => return Err(usage()),
    })
}

fn endpoint<'a>(host: &'a OsStr, port: &OsStr, user: &'a OsStr) -> Option<(&'a str, u16, &'a str)> {
    let (host, port, user) = (host.to_str()?, port.to_str()?.parse().ok()?, user.to_str()?);
    (!host.is_empty() && port != 0 && !user.is_empty()).then_some((host, port, user))
}

fn usage() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, USAGE)
}

struct RawMode;

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn read_secret(prompt: &str) -> io::Result<CryptoVec> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Err(io::Error::other(
            "SSH secret entry requires an interactive terminal",
        ));
    }
    enable_raw_mode()?;
    let mode = RawMode;
    eprint!("{prompt}");
    io::stderr().flush()?;
    let secret = read_secret_keys();
    drop(mode);
    eprintln!();
    secret
}

fn read_secret_keys() -> io::Result<CryptoVec> {
    let mut secret = CryptoVec::new();
    loop {
        let key = read_pressed_key()?;
        match key.code {
            KeyCode::Enter => return Ok(secret),
            KeyCode::Esc => return Err(cancelled()),
            KeyCode::Char('c' | 'd') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Err(cancelled());
            }
            KeyCode::Backspace => {
                let width = secret_text(&secret)?
                    .chars()
                    .next_back()
                    .map_or(0, char::len_utf8);
                secret.resize(secret.len() - width);
            }
            KeyCode::Char(c) if !c.is_control() => {
                secret.extend(c.encode_utf8(&mut [0; 4]).as_bytes());
            }
            _ => {}
        }
    }
}

fn read_pressed_key() -> io::Result<KeyEvent> {
    loop {
        if let Event::Key(key) = event::read()?
            && key.kind != KeyEventKind::Release
        {
            return Ok(key);
        }
    }
}

fn secret_text(secret: &CryptoVec) -> io::Result<&str> {
    std::str::from_utf8(secret).map_err(io::Error::other)
}

fn cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "SSH secret entry cancelled")
}
