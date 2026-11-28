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
use terminal_file_manager::{
    ssh::{Credentials, Session},
    ui,
};

const USAGE: &str = "cargo run --locked --example ssh_connect -- HOST PORT USER TRUSTED_HOST_KEY \
    (key PRIVATE_KEY | encrypted-key PRIVATE_KEY | agent | password)";

#[tokio::main]
async fn main() -> ExitCode {
    let result = connect().await;
    let result = match result {
        Ok(session) => {
            println!("Authenticated. Press Enter to disconnect.");
            let input = io::stdin().read_line(&mut String::new());
            input.and(session.disconnect().await)
        }
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

async fn connect() -> io::Result<Session> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let [host, port, user, trusted_key, method, extra @ ..] = args.as_slice() else {
        return Err(usage());
    };
    let (host, port, user) = endpoint(host, port, user).ok_or_else(usage)?;
    let trusted_key = load_public_key(trusted_key).map_err(|e| {
        io::Error::other(format!("Could not load trusted SSH host public key: {e}"))
    })?;
    let mut secret = CryptoVec::new();
    let credentials = credentials(method, extra, &mut secret)?;
    Session::connect(host, port, user, trusted_key, credentials).await
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
