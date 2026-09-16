use super::*;

#[tokio::test]
async fn last_arrival_wins_regardless_of_sender_sequence_and_duplicates_do_not_write() {
    let clipboard = FakeClipboard::new();
    let a = app(&clipboard).await;
    let mut b = RawPeer::connect(&a).await;
    let mut c = RawPeer::connect(&a).await;
    let old = b.text(100, "first").await;
    c.text(1, "second").await;
    b.text(2, "last arrival").await;
    assert_eq!(clipboard.current(), Content::Text("last arrival".into()));
    assert_eq!(clipboard.revision(), 3);
    b.connection
        .send(&Message::Text {
            id: old.clone(),
            target_epoch: b.epoch,
            text: "first".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        next_business(&mut b.connection).await,
        Message::Applied { id: old }
    );
    assert_eq!(clipboard.revision(), 3);
    assert_eq!(clipboard.current(), Content::Text("last arrival".into()));
    assert_eq!(a.history("".into(), 100, 0).await.unwrap().len(), 3);
    a.shutdown().await.unwrap();
}
#[tokio::test]
async fn receiver_epoch_rejects_text_queued_before_pause() {
    let clipboard = FakeClipboard::new();
    let a = app(&clipboard).await;
    let mut b = RawPeer::connect(&a).await;
    let old_epoch = b.epoch;
    a.set_settings(Settings {
        paused: true,
        ..a.status().settings
    })
    .await
    .unwrap();
    assert!(matches!(
        next_business(&mut b.connection).await,
        Message::State {
            accepting: false,
            ..
        }
    ));
    a.set_settings(Settings {
        paused: false,
        ..a.status().settings
    })
    .await
    .unwrap();
    let Message::State {
        epoch,
        accepting: true,
    } = next_business(&mut b.connection).await
    else {
        panic!("resumed state")
    };
    b.epoch = epoch;
    let id = MessageId {
        session: "a".repeat(32),
        sequence: 1,
    };
    b.connection
        .send(&Message::Text {
            id: id.clone(),
            target_epoch: old_epoch,
            text: "stale".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        next_business(&mut b.connection).await,
        Message::Rejected { id }
    );
    assert_eq!(clipboard.current(), Content::Empty);
    b.text(2, "fresh").await;
    assert_eq!(clipboard.current(), Content::Text("fresh".into()));
    a.shutdown().await.unwrap();
}
#[tokio::test]
async fn receiver_name_is_authenticated_metadata_and_duplicate_names_are_allowed() {
    let a = app(&FakeClipboard::new()).await;
    let mut b = RawPeer::connect(&a).await;
    let mut c = RawPeer::connect(&a).await;
    for peer in [&mut b, &mut c] {
        peer.connection
            .send(&Message::Device {
                device_name: "办公电脑".into(),
            })
            .await
            .unwrap();
    }
    wait_for(|| a.status().peers.iter().all(|p| p.device_name == "办公电脑")).await;
    assert_ne!(a.status().peers[0].noob_id, a.status().peers[1].noob_id);
    a.shutdown().await.unwrap();
}

#[tokio::test]
async fn simultaneous_incoming_messages_are_serialized_in_received_event_order() {
    let clipboard = FakeClipboard::new();
    let a = app(&clipboard).await;
    let mut b = RawPeer::connect(&a).await;
    let mut c = RawPeer::connect(&a).await;
    let mut events = a.subscribe();
    let bm = Message::Text {
        id: MessageId {
            session: "b".repeat(32),
            sequence: 1,
        },
        target_epoch: b.epoch,
        text: "one".into(),
    };
    let cm = Message::Text {
        id: MessageId {
            session: "c".repeat(32),
            sequence: 1,
        },
        target_epoch: c.epoch,
        text: "another message".into(),
    };
    let (sent_b, sent_c) = tokio::join!(b.connection.send(&bm), c.connection.send(&cm));
    sent_b.unwrap();
    sent_c.unwrap();
    let (ack_b, ack_c) = tokio::join!(
        next_business(&mut b.connection),
        next_business(&mut c.connection)
    );
    assert!(matches!(ack_b, Message::Applied { .. }));
    assert!(matches!(ack_c, Message::Applied { .. }));
    let mut arrivals = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let crate::Event::Received { source, bytes, .. } = event {
            arrivals.push((source, bytes));
        }
    }
    assert_eq!(arrivals.len(), 2);
    assert_ne!(arrivals[0].0, arrivals[1].0);
    let Content::Text(final_text) = clipboard.current() else {
        panic!("text expected")
    };
    assert_eq!(final_text.len(), arrivals.last().unwrap().1);
    assert_eq!(clipboard.revision(), 2);
    a.shutdown().await.unwrap();
}
