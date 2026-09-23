mod configuration;
mod lifecycle;
mod workflows;
use crate::ports::{ClipboardFuture, ClipboardPort};
use crate::*;
use nooboard_clipboard::{Origin, Payload, ReadState, Snapshot};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Notify, watch};
#[derive(Clone)]
pub(crate) struct Clipboard {
    state: Arc<watch::Sender<Option<Snapshot>>>,
    fail: Arc<AtomicBool>,
    block: Arc<AtomicBool>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}
impl Clipboard {
    pub(crate) fn new() -> Self {
        let (state, _) = watch::channel(Some(Snapshot {
            revision: 0,
            content: ReadState::Empty,
            origin: Origin::External,
        }));
        Self {
            state: Arc::new(state),
            fail: Default::default(),
            block: Default::default(),
            entered: Default::default(),
            release: Default::default(),
        }
    }
    pub(crate) fn copy(&self, payload: Payload) {
        self.publish(payload, Origin::External);
    }
    fn publish(&self, payload: Payload, origin: Origin) -> Snapshot {
        let revision = self.state.borrow().as_ref().unwrap().revision + 1;
        let snapshot = Snapshot {
            revision,
            content: ReadState::Ready(payload),
            origin,
        };
        self.state.send_replace(Some(snapshot.clone()));
        snapshot
    }
    pub(crate) fn text(&self) -> Option<String> {
        match &self.state.borrow().as_ref().unwrap().content {
            ReadState::Ready(Payload::Text(text)) => Some(text.clone()),
            _ => None,
        }
    }
}
impl ClipboardPort for Clipboard {
    fn subscribe(&self) -> watch::Receiver<Option<Snapshot>> {
        self.state.subscribe()
    }
    fn read(&self) -> ClipboardFuture<'_> {
        Box::pin(async { Ok(self.state.borrow().clone().unwrap()) })
    }
    fn write_content(&self, payload: Payload) -> ClipboardFuture<'_> {
        Box::pin(async move {
            if self.block.load(Ordering::SeqCst) {
                self.entered.notify_one();
                self.release.notified().await;
            }
            if self.fail.load(Ordering::SeqCst) {
                return Err(nooboard_clipboard::Error::InvalidData);
            }
            Ok(self.publish(payload, Origin::Application))
        })
    }
}
fn settings() -> Settings {
    Settings {
        listen_address: "127.0.0.1:0".into(),
        pairing_listen_address: "127.0.0.1:0".into(),
        discoverable: false,
        ..Settings::default()
    }
}
async fn start(clipboard: &Clipboard) -> (AppService, App) {
    start_with(
        clipboard,
        BackendConfig::Sqlite(SqliteOptions::in_memory()),
        settings(),
    )
    .await
    .unwrap()
}
async fn start_with(
    clipboard: &Clipboard,
    storage: BackendConfig,
    defaults: Settings,
) -> Result<(AppService, App)> {
    crate::runtime::startup::launch(crate::runtime::startup::Launch {
        options: Options {
            storage,
            profile: "tests".into(),
            default_receive_directory: None,
        },
        defaults,
        identity: nooboard_network::IdentityOptions::Ephemeral,
        clipboard_options: Default::default(),
        clipboard: Some(Arc::new(clipboard.clone())),
        clipboard_service: None,
    })
    .await
}
async fn bounded<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(15), future)
        .await
        .expect("operation timed out")
}
async fn wait(app: &App, condition: impl Fn(&AppSnapshot) -> bool) {
    let mut status = app.subscribe();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if condition(&status.borrow_and_update()) {
                return;
            }
            status.changed().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| panic!("snapshot timeout: {:?}", app.status()));
}
fn fixture(app: &App) -> PeerFixture {
    PeerFixture {
        noob_id: app.status().noob_id,
        certificate: app.certificate().to_vec(),
        confirmed_fingerprint: app.status().fingerprint,
        device_name: app.status().settings.device_name,
        address: Some(app.status().listen_address),
    }
}
pub(crate) async fn pair(a: &App, b: &App) {
    let (aid, bid) = (a.status().noob_id, b.status().noob_id);
    a.trust_peer(fixture(b)).await.unwrap();
    b.trust_peer(fixture(a)).await.unwrap();
    a.select_targets(vec![bid.clone()]).await.unwrap();
    b.select_targets(vec![aid.clone()]).await.unwrap();
    wait(a, |s| {
        s.status
            .peers
            .iter()
            .any(|p| p.noob_id == bid && p.accepting)
    })
    .await;
    wait(b, |s| {
        s.status
            .peers
            .iter()
            .any(|p| p.noob_id == aid && p.accepting)
    })
    .await;
}
async fn delivered(app: &App, id: &MessageId, state: DeliveryState) {
    wait(app, |s| {
        s.status
            .transfers
            .iter()
            .any(|t| &t.id == id && t.targets.iter().all(|d| d.state == state))
    })
    .await;
}

#[cfg(all(feature = "diagnostics", target_os = "macos"))]
pub(crate) async fn wait_for(mut condition: impl FnMut() -> bool) {
    bounded(async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
}
