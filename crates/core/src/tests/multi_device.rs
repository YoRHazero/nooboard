use super::*;

#[tokio::test]
async fn three_peers_fan_out_once_and_keep_independent_receipts() {
    let (ca, cb, cc) = (
        FakeClipboard::new(),
        FakeClipboard::new(),
        FakeClipboard::new(),
    );
    let (a, b, c) = (app(&ca).await, app(&cb).await, app(&cc).await);
    pair(&a, &b).await;
    pair(&a, &c).await;
    pair(&b, &c).await;
    for node in [&a, &b, &c] {
        automatic(node).await;
        assert_eq!(node.status().peers.len(), 2);
    }
    ca.text("fan out 🦀");
    wait_for(|| {
        cb.current() == ReadState::Ready(Payload::Text("fan out 🦀".into()))
            && cc.current() == cb.current()
    })
    .await;
    wait_for(|| {
        a.status().transfers.iter().any(|t| {
            t.targets.len() == 2 && t.targets.iter().all(|d| d.state == DeliveryState::Applied)
        })
    })
    .await;
    settle().await;
    assert_eq!(ca.revision(), 1);
    assert_eq!(cb.revision(), 1);
    assert_eq!(cc.revision(), 1);
    assert_eq!(a.status().transfers.len(), 1);
    assert!(b.status().transfers.is_empty());
    assert!(c.status().transfers.is_empty());
    cb.text("new local copy on B");
    wait_for(|| {
        ca.current() == ReadState::Ready(Payload::Text("new local copy on B".into()))
            && cc.current() == ca.current()
    })
    .await;
    settle().await;
    assert_eq!(ca.revision(), 2);
    assert_eq!(cb.revision(), 2);
    assert_eq!(cc.revision(), 2);
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
    c.shutdown().await.unwrap();
}
#[tokio::test]
async fn remote_updates_do_not_relay_to_a_third_device() {
    let (ca, cb, cc) = (
        FakeClipboard::new(),
        FakeClipboard::new(),
        FakeClipboard::new(),
    );
    let (a, b, c) = (app(&ca).await, app(&cb).await, app(&cc).await);
    pair(&a, &b).await;
    pair(&b, &c).await;
    for node in [&a, &b, &c] {
        automatic(node).await;
    }
    ca.text("only direct peers");
    wait_for(|| cb.current() == ca.current()).await;
    settle().await;
    assert_eq!(cc.current(), ReadState::Empty);
    assert!(b.status().transfers.is_empty());
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
    c.shutdown().await.unwrap();
}
#[tokio::test]
async fn manual_targets_are_separate_and_unpair_does_not_disconnect_others() {
    let (ca, cb, cc) = (
        FakeClipboard::new(),
        FakeClipboard::new(),
        FakeClipboard::new(),
    );
    let (a, b, c) = (app(&ca).await, app(&cb).await, app(&cc).await);
    pair(&a, &b).await;
    pair(&a, &c).await;
    assert!(a.status().peers.iter().all(|p| !p.settings.auto_send));
    ca.text("manual batch");
    let id = a
        .send_to(vec![b.status().noob_id, c.status().noob_id])
        .await
        .unwrap();
    wait_for(|| {
        outcome(&a, &id, &b.status().noob_id) == Some(DeliveryState::Applied)
            && outcome(&a, &id, &c.status().noob_id) == Some(DeliveryState::Applied)
    })
    .await;
    assert!(a.status().peers.iter().all(|p| !p.settings.auto_send));
    a.unpair(b.status().noob_id).await.unwrap();
    assert!(ready(&a, &c));
    assert_eq!(a.status().manual_targets, vec![c.status().noob_id]);
    ca.text("still connected");
    a.send_current().await.unwrap();
    wait_for(|| cc.current() == ca.current()).await;
    assert_eq!(
        cb.current(),
        ReadState::Ready(Payload::Text("manual batch".into()))
    );
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
    c.shutdown().await.unwrap();
}
#[tokio::test]
async fn reconnect_does_not_replay_and_one_offline_target_does_not_fail_a_batch() {
    let (ca, cb, cc) = (
        FakeClipboard::new(),
        FakeClipboard::new(),
        FakeClipboard::new(),
    );
    let (a, b, c) = (app(&ca).await, app(&cb).await, app(&cc).await);
    pair(&a, &b).await;
    pair(&a, &c).await;
    automatic(&a).await;
    let request_b = request(&a);
    b.unpair(a.status().noob_id).await.unwrap();
    wait_for(|| !ready(&a, &b)).await;
    ca.text("offline content");
    let id = a
        .send_to(vec![b.status().noob_id, c.status().noob_id])
        .await
        .unwrap();
    wait_for(|| outcome(&a, &id, &c.status().noob_id) == Some(DeliveryState::Applied)).await;
    assert_eq!(
        outcome(&a, &id, &b.status().noob_id),
        Some(DeliveryState::Offline)
    );
    b.trust_peer(request_b).await.unwrap();
    wait_for(|| ready(&a, &b) && ready(&b, &a)).await;
    settle().await;
    assert_eq!(cb.current(), ReadState::Empty);
    ca.text("fresh after reconnect");
    wait_for(|| cb.current() == ReadState::Ready(Payload::Text("fresh after reconnect".into())))
        .await;
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
    c.shutdown().await.unwrap();
}
#[tokio::test]
async fn missing_receipt_does_not_block_receiving_or_other_destinations() {
    let (ca, cb) = (FakeClipboard::new(), FakeClipboard::new());
    let (a, b) = (app(&ca).await, app(&cb).await);
    pair(&a, &b).await;
    let mut raw = RawPeer::connect(&a).await;
    ca.text("no receipt from raw peer");
    let id = a.send_to(vec![raw.id(), b.status().noob_id]).await.unwrap();
    let Message::Text { id: raw_id, .. } = next_business(&mut raw.connection).await else {
        panic!("text expected")
    };
    assert_eq!(raw_id, id);
    raw.text(1, "incoming while receipt is pending").await;
    wait_for(|| outcome(&a, &id, &b.status().noob_id) == Some(DeliveryState::Applied)).await;
    assert_eq!(
        ca.current(),
        ReadState::Ready(Payload::Text("incoming while receipt is pending".into()))
    );
    assert_eq!(
        outcome(&a, &id, &raw.id()),
        Some(DeliveryState::AwaitingReceipt)
    );
    let raw_id = raw.id();
    drop(raw);
    wait_for(|| outcome(&a, &id, &raw_id) == Some(DeliveryState::Unconfirmed)).await;
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_backpressured_writer_still_receives_and_does_not_block_healthy_peers() {
    let (ca, cb) = (FakeClipboard::new(), FakeClipboard::new());
    let (a, b) = (app(&ca).await, app(&cb).await);
    pair(&a, &b).await;
    let mut slow = RawPeer::connect(&a).await;
    let slow_id = slow.id();
    // The peer's bounded framing reader eventually fills because it never consumes our text.
    ca.text(&"x".repeat(nooboard_network::MAX_TEXT_BYTES));
    for _ in 0..64 {
        a.send_to(vec![slow_id.clone()]).await.unwrap();
    }
    assert!(
        a.status()
            .transfers
            .iter()
            .flat_map(|t| &t.targets)
            .any(|d| d.state == DeliveryState::Queued || d.state == DeliveryState::QueueFull)
    );
    let incoming = MessageId {
        session: "f".repeat(32),
        sequence: 1,
    };
    slow.connection
        .send(&Message::Text {
            id: incoming,
            target_epoch: slow.epoch,
            text: "receiving while our writer is congested".into(),
        })
        .await
        .unwrap();
    wait_for(|| {
        ca.current()
            == ReadState::Ready(Payload::Text(
                "receiving while our writer is congested".into(),
            ))
    })
    .await;
    let sent = a.send_to(vec![b.status().noob_id]).await.unwrap();
    wait_for(|| outcome(&a, &sent, &b.status().noob_id) == Some(DeliveryState::Applied)).await;
    assert_eq!(cb.current(), ca.current());
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}
