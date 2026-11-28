//! Direct SSH connections authenticated against a caller-supplied host public key.

mod sftp;

use std::{io, path::Path, sync::Arc, time::Duration};

use russh::{
    Disconnect, client,
    keys::{
        Algorithm, HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate,
        agent::{AgentIdentity, client::AgentClient},
        load_secret_key,
    },
};
use tokio::time::timeout;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// A single authentication method for a direct SSH connection.
pub enum Credentials<'a> {
    /// An OpenSSH private key file and, when needed, its passphrase.
    PrivateKey {
        /// File containing the private key.
        path: &'a Path,
        /// Passphrase for the encrypted key.
        passphrase: Option<&'a str>,
    },
    /// The local SSH agent.
    Agent,
    /// The password for the specified user.
    Password(&'a str),
}

/// An authenticated SSH session.
pub struct Session(client::Handle<PinnedHost>);

impl Session {
    /// Connect to the explicit endpoint, verify the server key and authenticate.
    ///
    /// The caller must obtain the trusted host public key independently.
    ///
    /// # Errors
    /// Returns an error for a transport failure, host-key mismatch, time limit
    /// or unsuccessful authentication.
    pub async fn connect(
        host: &str,
        port: u16,
        user: &str,
        trusted_key: PublicKey,
        credentials: Credentials<'_>,
    ) -> io::Result<Self> {
        let config = pinned_host_config(&trusted_key)?;
        let mut handle = timeout(
            CONNECT_TIMEOUT,
            client::connect(Arc::new(config), (host, port), PinnedHost(trusted_key)),
        )
        .await
        .map_err(|_| timed_out("SSH connection timed out"))?
        .map_err(connection_error)?;
        timeout(
            CONNECT_TIMEOUT,
            authenticate(&mut handle, user, credentials),
        )
        .await
        .map_err(|_| timed_out("SSH authentication timed out"))??;
        Ok(Self(handle))
    }

    /// Send SSH disconnect and wait for the local SSH transport to close.
    ///
    /// # Errors
    /// Returns an error if disconnect cannot be sent or the transport fails to close.
    pub async fn disconnect(self) -> io::Result<()> {
        timeout(DISCONNECT_TIMEOUT, close(self.0))
            .await
            .map_err(|_| timed_out("SSH disconnect timed out"))?
    }
}

struct PinnedHost(PublicKey);

impl client::Handler for PinnedHost {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        presented: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(
            matches!(presented, PublicKeyOrCertificate::PublicKey { key, .. }
            if self.0.key_data() == key.key_data()),
        )
    }
}

fn pinned_host_config(trusted_key: &PublicKey) -> io::Result<client::Config> {
    let mut config = client::Config::default();
    let pinned = trusted_key.algorithm();
    config
        .preferred
        .key
        .to_mut()
        .retain(|candidate| match candidate {
            Algorithm::Rsa { hash } => matches!(pinned, Algorithm::Rsa { .. }) && hash.is_some(),
            _ => &pinned == candidate,
        });
    if config.preferred.key.is_empty() {
        return Err(io::Error::other("unsupported trusted SSH host key type"));
    }
    Ok(config)
}

fn connection_error(error: russh::Error) -> io::Error {
    match error {
        russh::Error::UnknownKey => io::Error::other(
            "SSH host key mismatch. Verify the server and your trusted public key independently.",
        ),
        error => io::Error::other(format!("SSH connection failed: {error}")),
    }
}

async fn authenticate(
    handle: &mut client::Handle<PinnedHost>,
    user: &str,
    credentials: Credentials<'_>,
) -> io::Result<()> {
    let accepted = match credentials {
        Credentials::PrivateKey { path, passphrase } => {
            private_key(handle, user, path, passphrase).await?.success()
        }
        Credentials::Password(password) => handle
            .authenticate_password(user, password)
            .await
            .map_err(|e| io::Error::other(format!("SSH password authentication failed: {e}")))?
            .success(),
        Credentials::Agent => agent(handle, user).await?,
    };
    if accepted {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "SSH server rejected the credentials or requires another authentication factor",
        ))
    }
}

async fn private_key(
    handle: &mut client::Handle<PinnedHost>,
    user: &str,
    path: &Path,
    passphrase: Option<&str>,
) -> io::Result<client::AuthResult> {
    let key = load_secret_key(path, passphrase).map_err(|e| {
        io::Error::other(format!(
            "Could not load SSH private key {}: {e}",
            path.display()
        ))
    })?;
    let hash = if key.algorithm().is_rsa() {
        Some(rsa_hash(handle).await?.ok_or_else(|| {
            io::Error::other("SSH server offers no RSA/SHA-2 signature algorithm")
        })?)
    } else {
        None
    };
    handle
        .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
        .await
        .map_err(|e| io::Error::other(format!("SSH private-key authentication failed: {e}")))
}

async fn rsa_hash(handle: &client::Handle<PinnedHost>) -> io::Result<Option<HashAlg>> {
    let supported = handle
        .best_supported_rsa_hash()
        .await
        .map_err(|e| io::Error::other(format!("Could not determine SSH RSA algorithms: {e}")))?;
    // Some(None) is the legacy SHA-1 choice. When no list is available,
    // try SHA-256 once. An authentication failure never selects SHA-1.
    Ok(supported.unwrap_or(Some(HashAlg::Sha256)))
}

async fn agent(handle: &mut client::Handle<PinnedHost>, user: &str) -> io::Result<bool> {
    #[cfg(unix)]
    let agent = AgentClient::connect_env().await;
    #[cfg(windows)]
    let agent = AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await;

    let mut agent =
        agent.map_err(|e| io::Error::other(format!("Could not open local SSH agent: {e}")))?;
    let identities = agent
        .request_identities()
        .await
        .map_err(|e| io::Error::other(format!("Could not list SSH agent identities: {e}")))?;
    let mut keys = identities
        .into_iter()
        .filter_map(plain_agent_key)
        .peekable();
    if keys.peek().is_none() {
        return Err(io::Error::other(
            "The local SSH agent has no plain public keys. Load a key first.",
        ));
    }
    for key in keys {
        let hash = if key.algorithm().is_rsa() {
            let Some(hash) = rsa_hash(handle).await? else {
                continue;
            };
            Some(hash)
        } else {
            None
        };
        let result = handle
            .authenticate_publickey_with(user, key, hash, &mut agent)
            .await
            .map_err(|e| io::Error::other(format!("SSH agent authentication failed: {e}")))?;
        if result.success() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn plain_agent_key(identity: AgentIdentity) -> Option<PublicKey> {
    match identity {
        AgentIdentity::PublicKey { key, .. } => Some(key),
        AgentIdentity::Certificate { .. } => None,
    }
}

async fn close(handle: client::Handle<PinnedHost>) -> io::Result<()> {
    handle
        .disconnect(Disconnect::ByApplication, "Client finished", "")
        .await
        .map_err(|e| io::Error::other(format!("Could not send SSH disconnect: {e}")))?;
    match handle.await {
        // Russh reports Disconnect when its local disconnect loop has finished.
        Ok(()) | Err(russh::Error::Disconnect) => Ok(()),
        Err(e) => Err(io::Error::other(format!(
            "SSH transport failed while closing: {e}"
        ))),
    }
}

fn timed_out(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, message)
}
