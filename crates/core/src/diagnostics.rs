//! Explicit native harness. System credentials are never used here.
use crate::{App, AppService, BackendConfig, Options, Result, Settings, SqliteOptions};
use nooboard_clipboard::{
    Clipboard, ClipboardService, Options as ClipboardOptions, Payload, ReadState,
};
use std::{sync::Arc, time::Duration};
pub async fn trust_peer(app: &App, fixture: crate::PeerFixture) -> Result<String> {
    app.trust_peer(fixture).await
}
pub struct Session {
    pub app: App,
    service: AppService,
    input: Clipboard,
    input_service: ClipboardService,
}
impl Session {
    pub async fn start() -> Result<Self> {
        Self::with_settings(Settings {
            listen_address: "127.0.0.1:0".into(),
            pairing_listen_address: "127.0.0.1:0".into(),
            discoverable: false,
            ..Settings::default()
        })
        .await
    }
    pub async fn with_settings(settings: Settings) -> Result<Self> {
        let name = format!(
            "nooboard.diagnostic.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let options = ClipboardOptions {
            poll_interval: Duration::from_millis(30),
            #[cfg(target_os = "macos")]
            diagnostic_name: Some(name),
            ..Default::default()
        };
        #[cfg(not(target_os = "macos"))]
        let _ = name;
        let (clipboard_service, clipboard) = ClipboardService::start(options.clone()).await?;
        let (input_service, input) = match ClipboardService::start(options.clone()).await {
            Ok(value) => value,
            Err(e) => {
                let _ = clipboard_service.shutdown().await;
                return Err(e.into());
            }
        };
        let result = crate::runtime::startup::launch(crate::runtime::startup::Launch {
            options: Options {
                storage: BackendConfig::Sqlite(SqliteOptions::in_memory()),
                profile: "diagnostics".into(),
                default_receive_directory: None,
            },
            defaults: settings,
            identity: nooboard_network::IdentityOptions::Ephemeral,
            clipboard_options: options,
            clipboard: Some(Arc::new(clipboard)),
            clipboard_service: Some(clipboard_service),
        })
        .await;
        match result {
            Ok((service, app)) => Ok(Self {
                app,
                service,
                input,
                input_service,
            }),
            Err(e) => {
                let _ = input_service.shutdown().await;
                Err(e)
            }
        }
    }
    pub async fn copy(&self, text: String) -> Result<()> {
        self.input.write(Payload::Text(text)).await?;
        Ok(())
    }
    pub async fn read(&self) -> Result<Option<String>> {
        Ok(match self.input.read().await?.content {
            ReadState::Ready(Payload::Text(text)) => Some(text),
            _ => None,
        })
    }
    pub async fn shutdown(self) -> Result<()> {
        let app = self.service.shutdown().await;
        let input = self.input_service.shutdown().await;
        app?;
        input?;
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
                Payload::Image(image.clone()),
                Payload::Files(vec![picture.clone(), large.clone()]),
            ] {
                let expected = if matches!(content, Payload::Image(_)) {
                    ClipboardKind::Image
                } else {
                    ClipboardKind::Files
                };
                let revision = sender.app.snapshot().current.revision;
                sender.input.write(content.clone()).await.unwrap();
                wait_for(|| {
                    sender.app.snapshot().current.revision > revision
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
                    (Payload::Image(sent), ReadState::Ready(Payload::Image(received))) => {
                        assert_eq!(sent.rgba().unwrap(), received.rgba().unwrap())
                    }
                    (Payload::Files(_), ReadState::Ready(Payload::Files(paths))) => {
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
