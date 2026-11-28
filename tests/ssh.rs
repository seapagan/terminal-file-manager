use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use russh::{
    keys::{
        PrivateKey, PublicKey,
        ssh_key::{Cipher, Kdf, LineEnding, private::Ed25519Keypair},
    },
    server::{self, Auth, VerifiedPublicKey},
};
use terminal_file_manager::ssh::{Credentials, Session};
use tokio::{net::TcpListener, task::JoinHandle, time::timeout};

#[path = "ssh/sftp.rs"]
mod sftp;

const USER: &str = "test-user";
const PASSWORD: &str = "loopback-fixture-password";
const LIMIT: Duration = Duration::from_secs(10);

struct Server {
    attempts: Arc<AtomicUsize>,
    client_key: PublicKey,
    sftp: sftp::Subsystem,
}

impl server::Handler for Server {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        self.attempts.fetch_add(1, Ordering::Relaxed);
        Ok(if user == USER && password == PASSWORD {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        key: VerifiedPublicKey<'_>,
    ) -> Result<Auth, Self::Error> {
        Ok(
            if user == USER && key.public_key.key_data() == self.client_key.key_data() {
                Auth::Accept
            } else {
                Auth::reject()
            },
        )
    }

    async fn channel_open_session(
        &mut self,
        channel: russh::Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.sftp.channel = Some(channel);
        reply.accept().await;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: russh::ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.sftp.start(channel, name, session).await
    }

    async fn channel_close(
        &mut self,
        channel: russh::ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.sftp.channel_closed(channel);
        Ok(())
    }
}

fn fixture_key(seed: u8) -> PrivateKey {
    PrivateKey::new(Ed25519Keypair::from_seed(&[seed; 32]).into(), "test key").unwrap()
}

struct Peer {
    port: u16,
    host_key: PublicKey,
    attempts: Arc<AtomicUsize>,
    task: JoinHandle<Result<(), russh::Error>>,
}

impl Peer {
    async fn start() -> Self {
        Self::start_with(sftp::Subsystem::default()).await
    }

    async fn start_with(sftp: sftp::Subsystem) -> Self {
        let key = fixture_key(1);
        let host_key = key.public_key().clone();
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::ZERO,
            inactivity_timeout: Some(LIMIT),
            ..Default::default()
        });
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let attempts = Arc::new(AtomicUsize::new(0));
        let handler = Server {
            attempts: attempts.clone(),
            client_key: fixture_key(2).public_key().clone(),
            sftp,
        };
        let task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            server::run_stream(config, socket, handler).await?.await
        });
        Self {
            port,
            host_key,
            attempts,
            task,
        }
    }

    async fn connect(&self, credentials: Credentials<'_>) -> std::io::Result<Session> {
        timeout(
            LIMIT,
            Session::connect(
                "127.0.0.1",
                self.port,
                USER,
                self.host_key.clone(),
                credentials,
            ),
        )
        .await
        .expect("client failed to finish authenticating")
    }

    async fn server_finished(self) -> Result<(), russh::Error> {
        timeout(LIMIT, self.task)
            .await
            .expect("connection stayed open")
            .unwrap()
    }
}

#[tokio::test]
async fn password_authenticates_and_disconnects() {
    let peer = Peer::start().await;
    let session = peer.connect(Credentials::Password(PASSWORD)).await.unwrap();
    assert_eq!(peer.attempts.load(Ordering::Relaxed), 1);
    session.disconnect().await.unwrap();
    peer.server_finished().await.unwrap();
}

#[tokio::test]
async fn rejects_incorrect_credentials() {
    let peer = Peer::start().await;
    let error = peer
        .connect(Credentials::Password("wrong"))
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("rejected"));
    assert_eq!(peer.attempts.load(Ordering::Relaxed), 1);
    let _ = peer.server_finished().await;
}

#[tokio::test]
async fn mismatched_host_key_blocks_authentication() {
    let mut peer = Peer::start().await;
    peer.host_key = fixture_key(3).public_key().clone();
    let error = peer
        .connect(Credentials::Password(PASSWORD))
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("host key mismatch"));
    assert_eq!(peer.attempts.load(Ordering::Relaxed), 0);
    let _ = peer.server_finished().await;
}

#[tokio::test]
async fn authenticates_with_private_key_file() {
    authenticate_key(fixture_key(2), None).await;
}

#[tokio::test]
async fn authenticates_with_encrypted_private_key_file() {
    let passphrase = "fixture-key-passphrase";
    let encrypted = fixture_key(2)
        .encrypt_with(
            Cipher::Aes256Ctr,
            Kdf::Bcrypt {
                salt: vec![1; 16],
                rounds: 1,
            },
            2,
            passphrase,
        )
        .unwrap();
    authenticate_key(encrypted, Some(passphrase)).await;
}

async fn authenticate_key(key: PrivateKey, passphrase: Option<&str>) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("identity");
    key.write_openssh_file(&path, LineEnding::LF).unwrap();
    let peer = Peer::start().await;
    let session = peer
        .connect(Credentials::PrivateKey {
            path: &path,
            passphrase,
        })
        .await
        .unwrap();
    session.disconnect().await.unwrap();
    peer.server_finished().await.unwrap();
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod agent {
    use std::{
        env, io,
        path::Path,
        process::{Child, Command, Stdio},
    };

    use russh::keys::agent::client::AgentClient;
    use tokio::time::sleep;

    use super::{Credentials, Duration, LIMIT, Peer, fixture_key, timeout};

    const OWNED_SOCKET: &str = "TFM_TEST_OWNED_AGENT_SOCKET";

    struct OwnedAgent(Child);

    impl Drop for OwnedAgent {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn agent_authenticates_and_disconnects() {
        // Keep the path within the Unix socket length limit on both hosts.
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let socket = root.path().join("agent.sock");
        let _agent = OwnedAgent(
            Command::new("ssh-agent")
                .args(["-D", "-a"])
                .arg(&socket)
                .current_dir(root.path())
                .env("HOME", root.path())
                .env_remove("SSH_AUTH_SOCK")
                .env_remove("SSH_AGENT_PID")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .spawn()
                .expect("the SSH agent regression requires OpenSSH ssh-agent on PATH"),
        );

        // Give the child its own agent environment without changing the test process.
        let output = Command::new(env::current_exe().unwrap())
            .args(["--ignored", "--exact", "agent::agent_child"])
            .current_dir(root.path())
            .env("HOME", root.path())
            .env("SSH_AUTH_SOCK", &socket)
            .env(OWNED_SOCKET, &socket)
            .env_remove("SSH_AGENT_PID")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    #[ignore = "Owned child entry point; exercised by agent_authenticates_and_disconnects"]
    async fn agent_child() {
        let socket = env::var_os(OWNED_SOCKET).expect("run the SSH agent regression parent");
        assert_eq!(env::var_os("SSH_AUTH_SOCK").as_ref(), Some(&socket));
        timeout(LIMIT, load_fixture_key(Path::new(&socket)))
            .await
            .expect("fixture agent did not become ready and accept the test key")
            .unwrap();

        let peer = Peer::start().await;
        let session = peer.connect(Credentials::Agent).await.unwrap();
        session.disconnect().await.unwrap();
        peer.server_finished().await.unwrap();
    }

    async fn load_fixture_key(path: &Path) -> Result<(), russh::keys::Error> {
        loop {
            match AgentClient::connect_uds(path).await {
                Ok(mut agent) => return agent.add_identity(&fixture_key(2), &[]).await,
                Err(russh::keys::Error::IO(error))
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                    ) =>
                {
                    sleep(Duration::from_millis(10)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }
}
