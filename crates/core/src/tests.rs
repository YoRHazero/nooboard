use crate::{
    App, Endpoint, Error, Mode, PairRequest, bootstrap,
    ports::{ClipboardFuture, ClipboardPort},
};
use nooboard_clipboard::{Content, Origin, Snapshot};
use nooboard_network::Identity;
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
async fn app(clipboard: &FakeClipboard) -> App {
    bootstrap::start_parts(
        Database::in_memory().unwrap(),
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
async fn pair(a: &App, b: &App) -> (PairRequest, PairRequest) {
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap().to_string();
    drop(socket);
    let request_a = PairRequest {
        certificate: b.certificate().to_vec(),
        confirmed_fingerprint: b.status().fingerprint,
        endpoint: Endpoint::Listen(address.clone()),
    };
    let request_b = PairRequest {
        certificate: a.certificate().to_vec(),
        confirmed_fingerprint: a.status().fingerprint,
        endpoint: Endpoint::Connect(address),
    };
    a.pair(PairRequest {
        certificate: request_a.certificate.clone(),
        confirmed_fingerprint: request_a.confirmed_fingerprint.clone(),
        endpoint: request_a.endpoint.clone(),
    })
    .await
    .unwrap();
    b.pair(PairRequest {
        certificate: request_b.certificate.clone(),
        confirmed_fingerprint: request_b.confirmed_fingerprint.clone(),
        endpoint: request_b.endpoint.clone(),
    })
    .await
    .unwrap();
    wait_for(|| a.status().peer_accepting && b.status().peer_accepting).await;
    (request_a, request_b)
}

#[tokio::test]
async fn history_is_independent_of_sending_and_filters_nontext() {
    let clipboard = FakeClipboard::new();
    clipboard.text("old clipboard");
    let app = app(&clipboard).await;
    assert!(app.history("".into(), 100, 0).await.unwrap().is_empty());
    clipboard.text(" 中文 🦀\r\n ");
    settle().await;
    let rows = app.history("".into(), 100, 0).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, " 中文 🦀\r\n ");
    for content in [Content::Sensitive, Content::Unsupported, Content::TooLarge] {
        clipboard.copy(content, Origin::External);
        settle().await;
    }
    assert_eq!(app.history("".into(), 100, 0).await.unwrap().len(), 1);
    let mut settings = app.status().settings;
    settings.paused = true;
    app.set_settings(settings.clone()).await.unwrap();
    clipboard.text("paused history");
    settle().await;
    assert_eq!(app.history("".into(), 100, 0).await.unwrap().len(), 2);
    assert!(matches!(app.send_current().await, Err(Error::Paused)));
    settings.history = false;
    app.set_settings(settings).await.unwrap();
    clipboard.text("not recorded");
    settle().await;
    assert_eq!(app.history("".into(), 100, 0).await.unwrap().len(), 2);
    app.copy_history(rows[0].id).await.unwrap();
    assert_eq!(clipboard.current(), Content::Text(rows[0].text.clone()));
    app.clear_history().await.unwrap();
    assert!(app.history("".into(), 100, 0).await.unwrap().is_empty());
    app.shutdown().await.unwrap();
}

#[tokio::test]
async fn auto_manual_pause_and_unpair_use_real_tls_without_echo() {
    let ca = FakeClipboard::new();
    let cb = FakeClipboard::new();
    let a = app(&ca).await;
    let b = app(&cb).await;
    pair(&a, &b).await;
    ca.text("manual");
    settle().await;
    assert_eq!(cb.current(), Content::Empty);
    a.send_current().await.unwrap();
    wait_for(|| cb.current() == Content::Text("manual".into())).await;
    let mut settings = a.status().settings;
    settings.mode = Mode::Automatic;
    a.set_settings(settings).await.unwrap();
    let mut settings = b.status().settings;
    settings.mode = Mode::Automatic;
    b.set_settings(settings.clone()).await.unwrap();
    ca.text("automatic 🦀");
    wait_for(|| cb.current() == Content::Text("automatic 🦀".into())).await;
    settle().await;
    // Remote application must not trigger another native clipboard write back to the sender.
    assert_eq!(ca.0.borrow().as_ref().unwrap().revision, 2);
    cb.text("reverse");
    wait_for(|| ca.current() == Content::Text("reverse".into())).await;

    settings.paused = true;
    b.set_settings(settings.clone()).await.unwrap();
    wait_for(|| !a.status().peer_accepting).await;
    ca.text("during pause");
    settle().await;
    assert_eq!(cb.current(), Content::Text("reverse".into()));
    settings.paused = false;
    b.set_settings(settings).await.unwrap();
    wait_for(|| a.status().peer_accepting).await;
    settle().await;
    assert_eq!(cb.current(), Content::Text("reverse".into()));
    // An explicit send of the same text remains a new operation.
    a.send_current().await.unwrap();
    wait_for(|| cb.current() == Content::Text("during pause".into())).await;
    a.unpair().await.unwrap();
    wait_for(|| !a.status().online && !b.status().online).await;
    assert!(a.status().peer_fingerprint.is_none());
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}

#[tokio::test]
async fn reconnect_does_not_replay_offline_text() {
    let ca = FakeClipboard::new();
    let cb = FakeClipboard::new();
    let a = app(&ca).await;
    let b = app(&cb).await;
    let (_, request_b) = pair(&a, &b).await;
    let mut settings = a.status().settings;
    settings.mode = Mode::Automatic;
    a.set_settings(settings).await.unwrap();
    b.unpair().await.unwrap();
    wait_for(|| !a.status().online).await;
    ca.text("offline");
    settle().await;
    b.pair(request_b).await.unwrap();
    wait_for(|| a.status().peer_accepting && b.status().peer_accepting).await;
    settle().await;
    assert_eq!(cb.current(), Content::Empty);
    ca.text("new copy after reconnect");
    wait_for(|| cb.current() == Content::Text("new copy after reconnect".into())).await;
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}

#[tokio::test]
async fn fingerprints_must_be_confirmed_and_peer_replacement_is_explicit() {
    let a = app(&FakeClipboard::new()).await;
    let b = Identity::generate().unwrap();
    let endpoint = Endpoint::Connect("127.0.0.1:1".into());
    assert!(matches!(
        a.pair(PairRequest {
            certificate: b.certificate().to_vec(),
            confirmed_fingerprint: "wrong".into(),
            endpoint: endpoint.clone()
        })
        .await,
        Err(Error::Fingerprint)
    ));
    a.pair(PairRequest {
        certificate: b.certificate().to_vec(),
        confirmed_fingerprint: b.fingerprint(),
        endpoint: endpoint.clone(),
    })
    .await
    .unwrap();
    let other = Identity::generate().unwrap();
    assert!(matches!(
        a.pair(PairRequest {
            certificate: other.certificate().to_vec(),
            confirmed_fingerprint: other.fingerprint(),
            endpoint
        })
        .await,
        Err(Error::AlreadyPaired)
    ));
    a.shutdown().await.unwrap();
}

#[tokio::test]
async fn concurrent_automatic_copies_converge_over_tls() {
    let ca = FakeClipboard::new();
    let cb = FakeClipboard::new();
    let a = app(&ca).await;
    let b = app(&cb).await;
    pair(&a, &b).await;
    for app in [&a, &b] {
        let mut settings = app.status().settings;
        settings.mode = Mode::Automatic;
        app.set_settings(settings).await.unwrap();
    }
    ca.text("copy on A");
    cb.text("copy on B");
    let expected = if a.status().fingerprint > b.status().fingerprint {
        "copy on A"
    } else {
        "copy on B"
    };
    wait_for(|| {
        ca.current() == Content::Text(expected.into())
            && cb.current() == Content::Text(expected.into())
    })
    .await;
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}

#[tokio::test]
async fn receiver_epoch_rejects_delayed_text_after_resume() {
    use nooboard_network::{Connection, Message, TlsConfig};
    async fn next_business(connection: &mut Connection) -> Message {
        loop {
            match connection.receive().await.unwrap() {
                Message::Ping => connection.send(&Message::Pong).await.unwrap(),
                Message::Pong => {}
                message => return message,
            }
        }
    }
    let clipboard = FakeClipboard::new();
    let a = app(&clipboard).await;
    let peer = Identity::generate().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let tls = TlsConfig::new(&peer, a.certificate()).unwrap();
    a.pair(PairRequest {
        certificate: peer.certificate().to_vec(),
        confirmed_fingerprint: peer.fingerprint(),
        endpoint: Endpoint::Connect(address),
    })
    .await
    .unwrap();
    let mut connection = tls
        .accept(listener.accept().await.unwrap().0)
        .await
        .unwrap();
    let original = next_business(&mut connection).await;
    let Message::State {
        epoch: old_epoch,
        accepting: true,
    } = original
    else {
        panic!("expected ready state")
    };
    connection
        .send(&Message::State {
            epoch: 1,
            accepting: true,
        })
        .await
        .unwrap();
    wait_for(|| a.status().peer_accepting).await;
    let mut settings = a.status().settings;
    settings.paused = true;
    a.set_settings(settings.clone()).await.unwrap();
    assert!(matches!(
        next_business(&mut connection).await,
        Message::State {
            accepting: false,
            ..
        }
    ));
    settings.paused = false;
    a.set_settings(settings).await.unwrap();
    let Message::State {
        epoch: new_epoch,
        accepting: true,
    } = next_business(&mut connection).await
    else {
        panic!("expected resumed state")
    };
    connection
        .send(&Message::Text {
            sequence: 1,
            target_epoch: old_epoch,
            text: "stale".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        next_business(&mut connection).await,
        Message::Rejected { sequence: 1 }
    );
    assert_eq!(clipboard.current(), Content::Empty);
    assert!(a.history("".into(), 100, 0).await.unwrap().is_empty());
    connection
        .send(&Message::Text {
            sequence: 2,
            target_epoch: new_epoch,
            text: "fresh".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        next_business(&mut connection).await,
        Message::Applied { sequence: 2 }
    );
    assert_eq!(clipboard.current(), Content::Text("fresh".into()));
    a.shutdown().await.unwrap();
}

#[tokio::test]
async fn settings_and_unpair_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let identity = Identity::generate().unwrap();
    let secret = identity.export_secret();
    let clipboard = FakeClipboard::new();
    let a = bootstrap::start_parts(
        Database::open(&path).unwrap(),
        identity,
        Box::new(clipboard.clone()),
    )
    .await
    .unwrap();
    let mut settings = a.status().settings;
    settings.mode = Mode::Automatic;
    settings.history = false;
    a.set_settings(settings.clone()).await.unwrap();
    let peer = Identity::generate().unwrap();
    a.pair(PairRequest {
        certificate: peer.certificate().to_vec(),
        confirmed_fingerprint: peer.fingerprint(),
        endpoint: Endpoint::Connect("127.0.0.1:1".into()),
    })
    .await
    .unwrap();
    a.shutdown().await.unwrap();
    let b = bootstrap::start_parts(
        Database::open(&path).unwrap(),
        Identity::from_secret(&secret).unwrap(),
        Box::new(clipboard.clone()),
    )
    .await
    .unwrap();
    assert_eq!(b.status().settings, settings);
    assert_eq!(b.status().peer_fingerprint, Some(peer.fingerprint()));
    b.unpair().await.unwrap();
    b.shutdown().await.unwrap();
    let c = bootstrap::start_parts(
        Database::open(path).unwrap(),
        Identity::from_secret(&secret).unwrap(),
        Box::new(clipboard),
    )
    .await
    .unwrap();
    assert!(c.status().peer_fingerprint.is_none());
    c.shutdown().await.unwrap();
}
