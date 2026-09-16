mod history;
mod multi_device;
mod onboarding;
mod persistence;
mod receiving;
mod snapshots;
use crate::{
    App, DeliveryState, Error, Mode, PeerSettings, Settings, VerifiedPeer, bootstrap,
    ports::{ClipboardFuture, ClipboardPort},
};
use nooboard_clipboard::{Content, Origin, Snapshot};
use nooboard_network::{Connection, Identity, Message, MessageId, TlsConfig};
use nooboard_storage::Database;
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;

#[derive(Clone)]
struct FakeClipboard(Arc<watch::Sender<nooboard_clipboard::Result<Snapshot>>>);
impl FakeClipboard {
    fn new() -> Self {
        let (sender, _) = watch::channel(Ok(Snapshot {
            revision: 0,
            content: Content::Empty,
            origin: Origin::External,
        }));
        Self(Arc::new(sender))
    }
    fn copy(&self, content: Content, origin: Origin) {
        let revision = self.0.borrow().as_ref().unwrap().revision + 1;
        let _ = self.0.send_replace(Ok(Snapshot {
            revision,
            content,
            origin,
        }));
    }
    fn text(&self, text: &str) {
        self.copy(Content::Text(text.into()), Origin::External);
    }
    fn current(&self) -> Content {
        self.0.borrow().as_ref().unwrap().content.clone()
    }
    fn revision(&self) -> u64 {
        self.0.borrow().as_ref().unwrap().revision
    }
}
impl ClipboardPort for FakeClipboard {
    fn subscribe(&self) -> watch::Receiver<nooboard_clipboard::Result<Snapshot>> {
        self.0.subscribe()
    }
    fn read(&self) -> ClipboardFuture<'_> {
        Box::pin(async { self.0.borrow().clone() })
    }
    fn write(&self, text: String) -> ClipboardFuture<'_> {
        Box::pin(async move {
            self.copy(Content::Text(text), Origin::Application);
            self.0.borrow().clone()
        })
    }
}
fn test_database() -> Database {
    let mut db = Database::in_memory().unwrap();
    db.set_setting(
        "settings",
        &serde_json::to_vec(&Settings {
            listen_address: "127.0.0.1:0".into(),
            pairing_listen_address: "127.0.0.1:0".into(),
            discoverable: false,
            device_name: "同名设备".into(),
            ..Settings::default()
        })
        .unwrap(),
    )
    .unwrap();
    db
}
async fn app(clipboard: &FakeClipboard) -> App {
    bootstrap::start_parts(
        test_database(),
        Identity::generate().unwrap(),
        Box::new(clipboard.clone()),
    )
    .await
    .unwrap()
}
async fn wait_for(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(8), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("condition should become true");
}
async fn settle() {
    tokio::time::sleep(Duration::from_millis(100)).await;
}
fn request(peer: &App) -> VerifiedPeer {
    VerifiedPeer {
        certificate: peer.certificate().to_vec(),
        confirmed_fingerprint: peer.status().fingerprint,
        device_name: peer.status().settings.device_name,
        address: Some(peer.status().listen_address),
    }
}
fn ready(app: &App, peer: &App) -> bool {
    app.status()
        .peers
        .iter()
        .any(|p| p.noob_id == peer.status().noob_id && p.online && p.accepting)
}
async fn pair(a: &App, b: &App) {
    let (ar, br) = tokio::join!(a.trust_peer(request(b)), b.trust_peer(request(a)));
    ar.unwrap();
    br.unwrap();
    a.select_targets(vec![b.status().noob_id]).await.unwrap();
    b.select_targets(vec![a.status().noob_id]).await.unwrap();
    wait_for(|| ready(a, b) && ready(b, a)).await;
    settle().await;
}
async fn automatic(app: &App) {
    for p in app.status().peers {
        app.configure_peer(
            p.noob_id,
            PeerSettings {
                auto_send: true,
                ..p.settings
            },
        )
        .await
        .unwrap();
    }
    app.set_settings(Settings {
        mode: Mode::Automatic,
        ..app.status().settings
    })
    .await
    .unwrap();
}
fn outcome(app: &App, id: &MessageId, peer: &str) -> Option<DeliveryState> {
    app.status()
        .transfers
        .iter()
        .find(|t| t.id == *id)
        .and_then(|t| t.targets.iter().find(|d| d.noob_id == peer))
        .map(|d| d.state)
}
async fn next_business(connection: &mut Connection) -> Message {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            match connection.receive().await.unwrap() {
                Message::Ping => connection.send(&Message::Pong).await.unwrap(),
                Message::Pong | Message::Device { .. } => {}
                message => return message,
            }
        }
    })
    .await
    .unwrap()
}
struct RawPeer {
    identity: Identity,
    connection: Connection,
    epoch: u64,
}
impl RawPeer {
    async fn connect(app: &App) -> Self {
        let identity = Identity::generate().unwrap();
        app.trust_peer(VerifiedPeer {
            certificate: identity.certificate().to_vec(),
            confirmed_fingerprint: identity.fingerprint(),
            device_name: "raw".into(),
            address: None,
        })
        .await
        .unwrap();
        let mut connection = TlsConfig::new(&identity, app.certificate())
            .unwrap()
            .connect(&app.status().listen_address)
            .await
            .unwrap();
        let Message::State { epoch, .. } = next_business(&mut connection).await else {
            panic!("state expected")
        };
        connection
            .send(&Message::State {
                epoch: 1,
                accepting: true,
            })
            .await
            .unwrap();
        wait_for(|| {
            app.status()
                .peers
                .iter()
                .any(|p| p.noob_id == identity.noob_id().unwrap() && p.accepting)
        })
        .await;
        Self {
            identity,
            connection,
            epoch,
        }
    }
    fn id(&self) -> String {
        self.identity.noob_id().unwrap()
    }
    async fn text(&mut self, sequence: u64, text: &str) -> MessageId {
        let id = MessageId {
            session: self.id()[..32].into(),
            sequence,
        };
        self.connection
            .send(&Message::Text {
                id: id.clone(),
                target_epoch: self.epoch,
                text: text.into(),
            })
            .await
            .unwrap();
        assert_eq!(
            next_business(&mut self.connection).await,
            Message::Applied { id: id.clone() }
        );
        id
    }
}
