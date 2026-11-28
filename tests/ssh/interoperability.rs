//! Opt-in contract for independently provisioned SSH/SFTP servers.
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use portable_pty::CommandBuilder;
use russh::keys::{PublicKey, load_public_key};
use russh_sftp::{client::fs::ReadDir, protocol::FileType};
use terminal_file_manager::ssh::{Credentials, Session};

#[path = "../terminal_driver.rs"]
mod terminal_driver;

struct Fixture {
    root: PathBuf,
    host: String,
    port: u16,
    user: String,
    host_key: PublicKey,
}

impl Fixture {
    fn from_env() -> Self {
        let root = PathBuf::from(
            env::var_os("TFM_SSH_INTEROP_ROOT").expect("use a tests/fixtures/ssh launcher"),
        );
        Self {
            host: env::var("TFM_SSH_INTEROP_HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            host_key: load_public_key(root.join("fixture/host.pub")).unwrap(),
            port: env::var("TFM_SSH_INTEROP_PORT").unwrap().parse().unwrap(),
            user: env::var("TFM_SSH_INTEROP_USER").unwrap_or_else(|_| "tfm".into()),
            root,
        }
    }

    async fn connect(&self, credentials: Credentials<'_>) -> io::Result<Session> {
        Session::connect(
            &self.host,
            self.port,
            &self.user,
            self.host_key.clone(),
            credentials,
        )
        .await
    }

    async fn key_login(&self) -> Session {
        self.connect(key(&self.root.join("client"), None))
            .await
            .unwrap()
    }
}

fn key<'a>(path: &'a Path, passphrase: Option<&'a str>) -> Credentials<'a> {
    Credentials::PrivateKey { path, passphrase }
}

#[tokio::test]
#[ignore = "requires an independently provisioned server; use a tests/fixtures/ssh launcher"]
async fn authentication() {
    let fixture = Fixture::from_env();
    let client = fixture.root.join("client");
    let encrypted = fixture.root.join("encrypted");
    let passphrase = env::var("TFM_SSH_INTEROP_PASSPHRASE").unwrap();
    let mut credentials = vec![key(&client, None), key(&encrypted, Some(&passphrase))];
    if enabled("AGENT") {
        credentials.push(Credentials::Agent);
    }
    let password = if enabled("PASSWORD") {
        Some(fs::read_to_string(fixture.root.join("fixture/password")).unwrap())
    } else {
        None
    };
    if let Some(password) = &password {
        credentials.push(Credentials::Password(password));
        assert_error(
            fixture.connect(Credentials::Password("wrong")).await,
            "rejected",
        );
    }
    for credentials in credentials {
        fixture
            .connect(credentials)
            .await
            .unwrap()
            .disconnect()
            .await
            .unwrap();
    }
    assert_error(
        fixture
            .connect(key(&fixture.root.join("wrong"), None))
            .await,
        "rejected",
    );
}

fn enabled(capability: &str) -> bool {
    env::var(format!("TFM_SSH_INTEROP_{capability}")).as_deref() != Ok("0")
}

#[tokio::test]
#[ignore = "requires an independently provisioned server; use a tests/fixtures/ssh launcher"]
async fn host_identity() {
    let fixture = Fixture::from_env();
    let wrong = load_public_key(fixture.root.join("wrong.pub")).unwrap();
    let result = Session::connect(
        &fixture.host,
        fixture.port,
        &fixture.user,
        wrong,
        key(&fixture.root.join("client"), None),
    )
    .await;
    assert_error(result, "host key mismatch");
}

fn assert_error<T>(result: io::Result<T>, message: &str) {
    let error = result
        .err()
        .expect("expected an actionable SSH/SFTP failure");
    assert!(error.to_string().contains(message), "{error}");
}

#[tokio::test]
#[ignore = "requires an independently provisioned server; use a tests/fixtures/ssh launcher"]
async fn listings() {
    let fixture = Fixture::from_env();
    let ssh = fixture.key_login().await;
    let (home, initial) = ssh.list_sftp(None).await.unwrap();
    assert!(home.as_bytes().starts_with(b"/"));
    assert!(
        initial
            .into_iter()
            .any(|row| row.file_name().as_bytes() == b"suite")
    );

    let (_, entries) = ssh.list_sftp(Some(b"suite")).await.unwrap();
    listing_metadata(entries);
    let (_, link) = ssh.list_sftp(Some(b"suite/linked-dir")).await.unwrap();
    assert!(
        link.into_iter()
            .any(|row| row.file_name().as_bytes() == b"child-marker")
    );
    let (_, mut empty) = ssh.list_sftp(Some(b"suite/empty")).await.unwrap();
    assert!(empty.next().is_none());

    for (path, message) in [
        (b"suite/denied".as_slice(), "Permission denied"),
        (b"suite/nonexistent".as_slice(), "No such file"),
    ] {
        assert_error(ssh.list_sftp(Some(path)).await, message);
    }
    let (_, again) = ssh.list_sftp(Some(b"suite")).await.unwrap();
    assert!(
        again
            .into_iter()
            .any(|row| row.file_name().as_bytes() == b"plain-file")
    );
    ssh.disconnect().await.unwrap();
}

fn listing_metadata(entries: ReadDir) {
    let entries: Vec<_> = entries.collect();
    let row = |name: &[u8]| {
        entries
            .iter()
            .find(|entry| entry.file_name().as_bytes() == name)
            .unwrap_or_else(|| panic!("missing remote entry {name:?}"))
    };
    let plain = row(b"plain-file");
    let attrs = plain.metadata();
    assert_eq!(plain.file_type(), FileType::File);
    assert_eq!(attrs.size, Some(8));
    if enabled("POSIX_MODE") {
        assert_eq!(attrs.permissions.unwrap() & 0o777, 0o640);
    }
    assert_eq!(attrs.mtime, Some(1_700_000_000));
    assert!(attrs.atime.is_some());
    assert!(attrs.uid.is_some() && attrs.gid.is_some());

    let (name, path) = if enabled("RAW_NAME") {
        (b"byte-\xff".as_slice(), b"suite/byte-\xff".as_slice())
    } else {
        ("byte-界".as_bytes(), "suite/byte-界".as_bytes())
    };
    assert_eq!(row(name).path().as_bytes(), path);
    for link in [b"linked-dir".as_slice(), b"linked-file", b"broken-link"] {
        assert_eq!(row(link).file_type(), FileType::Symlink);
    }
    if env::var("TFM_SSH_INTEROP_MISSING_METADATA").is_ok_and(|value| value == "1") {
        let attrs = row(b"missing-metadata").metadata();
        assert_eq!(attrs.size, None);
        assert_eq!(
            (attrs.uid, attrs.gid, attrs.atime, attrs.mtime),
            (None, None, None, None)
        );
    }
}

#[test]
#[ignore = "requires an independently provisioned server; use a tests/fixtures/ssh launcher"]
fn browsing() {
    let fixture = Fixture::from_env();
    let mut command = CommandBuilder::new(env::var_os("TFM_SSH_INTEROP_EXAMPLE").unwrap());
    command.args([&fixture.host, &fixture.port.to_string(), &fixture.user]);
    command.arg(fixture.root.join("fixture/host.pub"));
    command.arg("key");
    command.arg(fixture.root.join("client"));
    command.args(["--pane", "suite"]);
    let home = fixture.root.join("local");
    command.cwd(&home);
    command.env("HOME", &home);
    command.env("USERPROFILE", &home);
    command.env("APPDATA", home.join("config"));
    command.env("LOCALAPPDATA", home.join("data"));
    command.env("XDG_CONFIG_HOME", home.join("config"));
    command.env("XDG_DATA_HOME", home.join("data"));
    command.env("XDG_CACHE_HOME", home.join("cache"));
    command.env("GIT_CONFIG_GLOBAL", home.join("gitconfig"));
    command.env("GIT_CONFIG_NOSYSTEM", "1");
    command.env("TERM", "xterm-256color");
    command.env("NO_COLOR", "1");
    command.env_remove("VTE_VERSION");
    let output = terminal_driver::drive(
        command,
        &[
            ("plain-file", "\t\r"),
            ("child-marker", "\u{7f}"),
            ("plain-file", "q"),
        ],
        false,
    );
    assert!(output.contains("local-marker"));
    assert!(output.contains("Disconnected."));
}
