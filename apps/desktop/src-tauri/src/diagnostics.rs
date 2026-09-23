//! Explicit debug-only end-to-end session: real core/TLS/SQLite, private macOS pasteboards.
use nooboard_core::{
    App,
    diagnostics::{PeerFixture, Session, trust_peer},
};
pub struct Diagnostics {
    pub local: Session,
    peers: Vec<Session>,
    sequence: u64,
}
fn request(app: &App) -> PeerFixture {
    PeerFixture {
        noob_id: app.status().noob_id,
        certificate: app.certificate().to_vec(),
        confirmed_fingerprint: app.status().fingerprint,
        device_name: app.status().settings.device_name,
        address: Some(app.status().listen_address),
    }
}
impl Diagnostics {
    pub async fn start() -> nooboard_core::Result<Self> {
        let defaults = nooboard_core::Settings {
            listen_address: "127.0.0.1:0".into(),
            pairing_listen_address: "127.0.0.1:0".into(),
            discoverable: false,
            ..Default::default()
        };
        let local = Session::with_settings(nooboard_core::Settings {
            device_name: "本机原生验证".into(),
            ..defaults.clone()
        })
        .await?;
        let mut peers = Vec::new();
        for name in ["验证设备 A", "验证设备 B"] {
            let peer = Session::with_settings(nooboard_core::Settings {
                device_name: name.into(),
                ..defaults.clone()
            })
            .await?;
            trust_peer(&local.app, request(&peer.app)).await?;
            trust_peer(&peer.app, request(&local.app)).await?;
            peers.push(peer);
        }
        local
            .app
            .select_targets(peers.iter().map(|p| p.app.status().noob_id).collect())
            .await?;
        local
            .copy("原生接入验证\n这段文字位于隔离的测试剪贴板中。".into())
            .await?;
        Ok(Self {
            local,
            peers,
            sequence: 0,
        })
    }
    pub async fn action(&mut self, action: &str) -> Result<(), crate::errors::UiError> {
        self.sequence += 1;
        match action {
            "copy" => self
                .local
                .copy(format!(
                    "本机原生复制 {}\nReact → Tauri → core",
                    self.sequence
                ))
                .await
                .map_err(crate::errors::core),
            "receive" => {
                let peer = &self.peers[0];
                peer.copy(format!(
                    "远端原生来信 {}\n通过真实 TLS 连接收到的文字。",
                    self.sequence
                ))
                .await
                .map_err(crate::errors::core)?;
                peer.app
                    .send_to(vec![self.local.app.status().noob_id])
                    .await
                    .map_err(crate::errors::core)?;
                Ok(())
            }
            _ => Err(crate::errors::ui("generic")),
        }
    }
    pub async fn shutdown(self) {
        for peer in self.peers {
            let _ = peer.shutdown().await;
        }
        let _ = self.local.shutdown().await;
    }
}
