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
        let app =
            crate::bootstrap::start_parts(database, identity, Box::new(clipboard), None).await?;
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::{
        ClipboardKind, ContentStage,
        tests::{pair, wait_for},
    };
    use nooboard_clipboard::{ImageData, ImageEncoding};

    #[tokio::test]
    #[ignore = "two private macOS pasteboards and real localhost TLS; not cross-device GUI validation"]
    async fn native_images_and_files_both_directions() {
        let a = Session::start().await.unwrap();
        let b = Session::start().await.unwrap();
        let root_a = tempfile::tempdir().unwrap();
        let root_b = tempfile::tempdir().unwrap();
        for (session, directory) in [(&a, &root_a), (&b, &root_b)] {
            session
                .app
                .set_settings(Settings {
                    receive_directory: Some(directory.path().into()),
                    ..session.app.status().settings
                })
                .await
                .unwrap();
        }
        pair(&a.app, &b.app).await;
        let png = include_bytes!("tests/fixtures/alpha.png").to_vec();
        let image = ImageData::new(ImageEncoding::Png, png.clone()).unwrap();
        let source = tempfile::tempdir().unwrap();
        let picture = source.path().join("图片 # 1.png");
        let large = source.path().join("sample.bin");
        std::fs::write(&picture, &png).unwrap();
        let data: Vec<_> = (0..16 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        std::fs::write(&large, &data).unwrap();
        for (sender, receiver) in [(&a, &b), (&b, &a)] {
            for content in [
                Content::Image(image.clone()),
                Content::Files(vec![picture.clone(), large.clone()]),
            ] {
                let expected = if matches!(content, Content::Image(_)) {
                    ClipboardKind::Image
                } else {
                    ClipboardKind::Files
                };
                let revision = sender
                    .input
                    .write_content(content.clone())
                    .await
                    .unwrap()
                    .revision;
                wait_for(|| {
                    sender.app.snapshot().current.revision >= revision
                        && sender.app.snapshot().current.kind == expected
                })
                .await;
                let id = sender.app.send_current().await.unwrap();
                wait_for(|| {
                    sender
                        .app
                        .snapshot()
                        .content_transfers
                        .iter()
                        .any(|r| r.id == id && r.stage == ContentStage::Completed)
                })
                .await;
                match (content, receiver.input.read().await.unwrap().content) {
                    (Content::Image(sent), Content::Image(received)) => assert_eq!(
                        sent.decode().unwrap().to_rgba8(),
                        received.decode().unwrap().to_rgba8()
                    ),
                    (Content::Files(_), Content::Files(paths)) => {
                        assert_eq!(std::fs::read(&paths[0]).unwrap(), png);
                        assert_eq!(std::fs::read(&paths[1]).unwrap(), data);
                        assert_ne!(paths[0], picture);
                    }
                    _ => panic!("native clipboard semantics changed"),
                }
            }
        }
        assert!(
            a.app
                .history(String::new(), 20, 0)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            b.app
                .history(String::new(), 20, 0)
                .await
                .unwrap()
                .is_empty()
        );
        a.shutdown().await.unwrap();
        b.shutdown().await.unwrap();
    }
}
