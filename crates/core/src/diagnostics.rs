//! Opt-in native integration harness. Identities and SQLite live only in memory.
//! macOS uses a private pasteboard; Linux/Windows require an isolated test desktop.
use crate::{App, Result, Settings};
use nooboard_clipboard::{Clipboard, Content};
use nooboard_network::{Identity, MAX_TEXT_BYTES};
use nooboard_storage::Database;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Explicit fixture injection for transport tests; not part of normal application pairing.
pub use crate::model::VerifiedPeer as PeerFixture;
pub async fn trust_peer(app: &App, fixture: PeerFixture) -> Result<String> {
    app.trust_peer(fixture).await
}

pub struct Session {
    pub app: App,
    input: Clipboard,
}
impl Session {
    pub async fn start() -> Result<Self> {
        let (database, identity, clipboard, input) =
            tokio::task::spawn_blocking(|| -> Result<_> {
                let name = format!(
                    "nooboard.diagnostic.{}.{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                );
                let clipboard = Clipboard::open_diagnostic(
                    name.clone(),
                    Duration::from_millis(30),
                    MAX_TEXT_BYTES,
                )?;
                let input =
                    Clipboard::open_diagnostic(name, Duration::from_millis(30), MAX_TEXT_BYTES)?;
                let mut database = Database::in_memory()?;
                database.set_setting(
                    "settings",
                    &serde_json::to_vec(&Settings {
                        listen_address: "127.0.0.1:0".into(),
                        pairing_listen_address: "127.0.0.1:0".into(),
                        discoverable: false,
                        ..Settings::default()
                    })
                    .map_err(|_| crate::Error::Configuration)?,
                )?;
                Ok((database, Identity::generate()?, clipboard, input))
            })
            .await
            .map_err(|_| crate::Error::Stopped)??;
        let app = crate::bootstrap::start_parts(database, identity, Box::new(clipboard)).await?;
        Ok(Self { app, input })
    }
    /// A second native client simulates an external application copying text.
    pub async fn copy(&self, text: String) -> Result<()> {
        self.input.write_text(text).await?;
        Ok(())
    }
    pub async fn read(&self) -> Result<Option<String>> {
        Ok(match self.input.read().await?.content {
            Content::Text(text) => Some(text),
            _ => None,
        })
    }
    pub async fn shutdown(self) -> Result<()> {
        self.app.shutdown().await?;
        self.input.shutdown().await?;
        Ok(())
    }
}
