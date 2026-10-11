use std::{error::Error, sync::Arc, time::Duration};

use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use russh::keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn check_crypto() -> Result<()> {
    // Public fixture material, never a credential or production encryption key.
    let key = PrivateKey::from(Ed25519Keypair::from_seed(&[7; 32]));
    let signature = key.sign("tfm-m0", HashAlg::Sha256, b"probe")?;
    key.public_key().verify("tfm-m0", b"probe", &signature)?;
    assert!(
        key.public_key()
            .verify("tfm-m0", b"changed", &signature)
            .is_err()
    );
    let cipher = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &[9; 32]).unwrap());
    let mut bytes = b"ring-native-link".to_vec();
    cipher
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key([0; 12]),
            Aad::empty(),
            &mut bytes,
        )
        .unwrap();
    let plaintext = cipher
        .open_in_place(
            Nonce::assume_unique_for_key([0; 12]),
            Aad::empty(),
            &mut bytes,
        )
        .unwrap();
    assert_eq!(plaintext, b"ring-native-link");
    Ok(())
}

struct Client;
impl russh::client::Handler for Client {
    type Error = russh::Error;
}

async fn check_transport() -> Result<()> {
    // Only a banner exchange: this is not an SSH server or auth acceptance test.
    let (client, mut peer) = tokio::io::duplex(16_384);
    let peer_task = tokio::spawn(async move {
        peer.write_all(b"SSH-2.0-tfm-m0-probe\r\n").await?;
        let mut banner = Vec::new();
        BufReader::new(peer).read_until(b'\n', &mut banner).await?;
        assert!(banner.starts_with(b"SSH-2.0-"));
        Ok::<_, std::io::Error>(())
    });
    let connection = tokio::time::timeout(
        Duration::from_secs(5),
        russh::client::connect_stream(Arc::new(russh::client::Config::default()), client, Client),
    )
    .await?;
    assert!(connection.is_err());
    peer_task.await??;
    Ok(())
}

fn check_terminal() -> Result<()> {
    use ratatui::{TerminalOptions, Viewport, layout::Rect};
    let mut terminal = ratatui::Terminal::with_options(
        ratatui::backend::CrosstermBackend::new(Vec::new()),
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 12, 1)),
        },
    )?;
    terminal.draw(|frame| {
        frame.render_widget(ratatui::widgets::Paragraph::new("M0 probe"), frame.area())
    })?;
    let mut output = Vec::new();
    crossterm::queue!(output, crossterm::cursor::MoveTo(0, 0))?;
    assert!(!output.is_empty());
    Ok(())
}

fn main() -> Result<()> {
    check_crypto()?;
    check_terminal()?;
    yazi_sftp::probe::check_handle_bytes()?;
    yazi_sftp::probe::check_handle_requests()?;
    yazi_sftp::probe::check_path_bytes()?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(check_transport())?;
    println!("tfm M0 dependency probe: OK");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn crypto_and_terminal_smoke() {
        super::check_crypto().unwrap();
        super::check_terminal().unwrap();
    }

    #[tokio::test]
    async fn ssh_banner_smoke() {
        super::check_transport().await.unwrap();
    }
}
