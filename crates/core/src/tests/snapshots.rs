use super::*;
use crate::{ActivityKind, ClipboardKind};

#[tokio::test]
async fn network_snapshot_uses_live_ports_and_keeps_them_on_failed_rebind() {
    use std::net::SocketAddr;
    let app = app(&FakeClipboard::new()).await;
    let before = app.snapshot();
    assert_eq!(
        before.local_network.sync_port,
        before
            .status
            .listen_address
            .parse::<SocketAddr>()
            .unwrap()
            .port()
    );
    assert_eq!(
        before.local_network.pairing_port,
        before
            .onboarding
            .pairing_address
            .parse::<SocketAddr>()
            .unwrap()
            .port()
    );
    assert_ne!(before.local_network.pairing_port, 0);
    assert!(before.local_network.addresses.is_empty()); // Loopback-only diagnostic listeners.
    assert!(before.local_network.error.is_none());
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = occupied.local_addr().unwrap();
    let settings = Settings {
        pairing_listen_address: address.to_string(),
        ..app.status().settings
    };
    assert!(app.set_settings(settings.clone()).await.is_err());
    assert_eq!(
        app.snapshot().local_network.pairing_port,
        before.local_network.pairing_port
    );
    assert_eq!(app.status().settings.pairing_listen_address, "127.0.0.1:0");
    drop(occupied);
    app.set_settings(settings).await.unwrap();
    assert_eq!(app.snapshot().local_network.pairing_port, address.port());
    assert_eq!(
        app.snapshot().local_network.sync_port,
        before.local_network.sync_port
    );
    assert_eq!(
        app.snapshot().onboarding.pairing_address,
        address.to_string()
    );
    app.shutdown().await.unwrap();
}

#[tokio::test]
async fn snapshot_recovers_current_without_recording_startup_or_exposing_nontext() {
    let clipboard = FakeClipboard::new();
    clipboard.text("already on clipboard");
    let app = app(&clipboard).await;
    settle().await;
    let first = app.snapshot();
    assert_eq!(first.current.text.as_deref(), Some("already on clipboard"));
    assert!(first.activities.is_empty());
    assert!(app.history("".into(), 100, 0).await.unwrap().is_empty());
    let mut snapshots = app.subscribe_snapshots();
    clipboard.text("\n  第一行 🐦\n正文不出现在活动摘要");
    wait_for(|| !app.snapshot().activities.is_empty()).await;
    snapshots.changed().await.unwrap();
    let copied = snapshots.borrow_and_update().clone();
    assert_eq!(copied.session, first.session);
    assert!(copied.revision > first.revision);
    assert_eq!(copied.activities[0].summary, "第一行 🐦");
    assert!(copied.activities[0].kind == ActivityKind::Copied);
    assert!(copied.history_revision > first.history_revision);
    for (content, kind) in [
        (
            ReadState::Skipped(SkipReason::Sensitive),
            ClipboardKind::Sensitive,
        ),
        (
            ReadState::Skipped(SkipReason::TooLarge),
            ClipboardKind::TooLarge,
        ),
        (
            ReadState::Skipped(SkipReason::Unsupported),
            ClipboardKind::Unsupported,
        ),
        (ReadState::Empty, ClipboardKind::Empty),
    ] {
        clipboard.copy(content, Origin::External);
        wait_for(|| app.snapshot().current.kind == kind).await;
        assert!(app.snapshot().current.text.is_none());
        assert_eq!(app.snapshot().activities.len(), 1);
    }
    app.shutdown().await.unwrap();
}

#[tokio::test]
async fn snapshot_keeps_one_batch_and_remote_origin_after_multi_device_receipts() {
    let ca = FakeClipboard::new();
    let cb = FakeClipboard::new();
    let cc = FakeClipboard::new();
    let a = app(&ca).await;
    let b = app(&cb).await;
    let c = app(&cc).await;
    pair(&a, &b).await;
    pair(&a, &c).await;
    automatic(&b).await;
    automatic(&c).await;
    a.select_targets(vec![b.status().noob_id, c.status().noob_id])
        .await
        .unwrap();
    ca.text("一条消息\n两台接收");
    wait_for(|| !a.snapshot().activities.is_empty()).await;
    let id = a.send_current().await.unwrap();
    wait_for(|| {
        [&b, &c]
            .iter()
            .all(|p| outcome(&a, &id, &p.status().noob_id) == Some(DeliveryState::Applied))
    })
    .await;
    settle().await;
    let snapshot = a.snapshot();
    let sent = snapshot
        .activities
        .iter()
        .filter(|r| r.kind == ActivityKind::Sent)
        .collect::<Vec<_>>();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].message_id.as_ref(), Some(&id));
    assert_eq!(sent[0].summary, "一条消息");
    assert_eq!(snapshot.status.transfers[0].targets.len(), 2);
    for receiver in [&b, &c] {
        let snapshot = receiver.snapshot();
        assert_eq!(
            snapshot.current.source.as_deref(),
            Some(a.status().noob_id.as_str())
        );
        assert_eq!(snapshot.current.text.as_deref(), Some("一条消息\n两台接收"));
        assert_eq!(snapshot.activities.len(), 1);
        assert!(snapshot.activities[0].kind == ActivityKind::Received);
        assert!(snapshot.status.transfers.is_empty());
        assert_eq!(
            receiver
                .history_filtered("".into(), Some(false), 10, 0)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(
            receiver
                .history_filtered("".into(), Some(true), 10, 0)
                .await
                .unwrap()
                .is_empty()
        );
    }
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
    c.shutdown().await.unwrap();
}
