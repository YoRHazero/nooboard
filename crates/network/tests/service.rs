use nooboard_network::{NetworkService, *};
use std::{future::Future, time::Duration};

fn options(name: &str) -> Options {
    Options {
        identity: IdentityOptions::Ephemeral,
        device_name: name.into(),
        listen: "127.0.0.1:0".parse().unwrap(),
        pairing_listen: "127.0.0.1:0".parse().unwrap(),
        discovery: false,
        operation_timeout: Duration::from_secs(2),
        ..Options::default()
    }
}
async fn bounded<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(15), future)
        .await
        .expect("operation timed out")
}
async fn start(name: &str) -> (NetworkService, Network, NetworkEvents) {
    bounded(NetworkService::start(options(name))).await.unwrap()
}
async fn wait(service: &Network, condition: impl Fn(&NetworkStatus) -> bool) -> NetworkStatus {
    let mut status = service.subscribe();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let current = status.borrow_and_update().clone();
            if condition(&current) {
                return current;
            }
            status
                .changed()
                .await
                .unwrap_or_else(|_| panic!("service stopped: {:?}", status.borrow().state));
        }
    })
    .await
    .unwrap_or_else(|_| {
        let s = service.status();
        panic!(
            "state did not converge: {:?}, connections={:?}, transfers={:?}",
            s.state, s.connections, s.transfers
        )
    })
}
fn trust(service: &Network) -> TrustedPeer {
    let status = service.status();
    TrustedPeer {
        identity: status.identity,
        addresses: vec![status.listen_address],
    }
}
async fn link(a: &Network, b: &Network) {
    a.trust_peer(trust(b)).await.unwrap();
    b.trust_peer(trust(a)).await.unwrap();
    let aid = a.status().identity.id;
    let bid = b.status().identity.id;
    wait(a, |s| {
        s.connections.iter().any(|p| p.peer == bid && p.accepting)
    })
    .await;
    wait(b, |s| {
        s.connections.iter().any(|p| p.peer == aid && p.accepting)
    })
    .await;
}
async fn event(events: &mut NetworkEvents) -> NetworkEvent {
    bounded(events.next_event()).await.unwrap()
}
async fn stage(service: &Network, id: &TransferId, peer: &str, expected: TransferStage) {
    wait(service, |s| {
        s.transfers
            .iter()
            .any(|t| &t.id == id && t.peer == peer && !t.incoming && t.stage == expected)
    })
    .await;
}

#[tokio::test]
async fn facade_delivers_exact_text_only_reports_applied_after_application_and_closes_listeners() {
    let (a_service, a, _a_events) = start("sender").await;
    let (b_service, b, mut b_events) = start("receiver").await;
    link(&a, &b).await;
    let peer = b.status().identity.id.clone();
    let text = "  中文🦀\r\n private content ";
    let id = a
        .send(SendRequest::text(vec![peer.clone()], text))
        .await
        .unwrap();
    let NetworkEvent::ContentReady {
        id: incoming,
        content: ReceivedContent::Text(received),
        ..
    } = event(&mut b_events).await
    else {
        panic!("expected text")
    };
    assert_eq!(received, text);
    assert!(
        !a.status()
            .transfers
            .iter()
            .any(|t| t.stage == TransferStage::Applied)
    );
    assert!(!format!("{:?}", b.status()).contains(text));
    b.complete_incoming(incoming, ApplicationOutcome::Applied)
        .await
        .unwrap();
    stage(&a, &id, &peer, TransferStage::Applied).await;
    let address = a.status().listen_address;
    let pairing = a.status().pairing_address;
    let status = a.subscribe();
    bounded(a_service.shutdown()).await.unwrap();
    assert_eq!(status.borrow().state, ServiceState::Stopped);
    let _listener = tokio::net::TcpListener::bind(address).await.unwrap();
    let _pairing = tokio::net::TcpListener::bind(pairing).await.unwrap();
    bounded(b_service.shutdown()).await.unwrap();
}

#[tokio::test]
async fn real_pairing_retries_code_and_requires_persistence_before_activating_trust() {
    let (a_service, a, mut a_events) = start("a").await;
    let (b_service, b, mut b_events) = start("b").await;
    let aid = a
        .start_pairing(vec![b.status().pairing_address])
        .await
        .unwrap();
    let NetworkEvent::PairingOffered { id: bid, .. } = event(&mut b_events).await else {
        panic!("expected pairing offer")
    };
    b.accept_pairing(bid.clone()).await.unwrap();
    let snapshot = wait(&b, |s| {
        s.pairings
            .iter()
            .any(|p| p.id == bid && p.stage == PairingStage::ShowingCode)
    })
    .await;
    let code = snapshot
        .pairings
        .iter()
        .find(|p| p.id == bid)
        .unwrap()
        .code
        .clone()
        .unwrap();
    assert!(!format!("{snapshot:?}").contains(&code));
    wait(&a, |s| {
        s.pairings
            .iter()
            .any(|p| p.id == aid && p.stage == PairingStage::EnteringCode)
    })
    .await;
    a.submit_pairing_code(
        aid.clone(),
        if code == "00000000" {
            "11111111"
        } else {
            "00000000"
        }
        .into(),
    )
    .await
    .unwrap();
    wait(&a, |s| {
        s.pairings
            .iter()
            .any(|p| p.id == aid && p.stage == PairingStage::EnteringCode && p.attempts_left == 2)
    })
    .await;
    a.submit_pairing_code(aid.clone(), code).await.unwrap();
    let NetworkEvent::PairingVerified { id, peer } = event(&mut b_events).await else {
        panic!("expected verified record")
    };
    assert_eq!(id, bid);
    assert_eq!(peer.identity.id, a.status().identity.id);
    assert!(b.status().connections.is_empty());
    // Model the caller persisting the public record as text, without a storage dependency.
    let document = serde_json::to_string(&peer).unwrap();
    assert_eq!(
        serde_json::from_str::<TrustedPeer>(&document).unwrap(),
        peer
    );
    b.complete_pairing(id, true).await.unwrap();
    let NetworkEvent::PairingVerified { id, .. } = event(&mut a_events).await else {
        panic!("expected verified record")
    };
    a.complete_pairing(id, true).await.unwrap();
    wait(&a, |s| {
        s.pairings
            .iter()
            .any(|p| p.id == aid && p.stage == PairingStage::Completed)
    })
    .await;
    wait(&b, |s| {
        s.pairings
            .iter()
            .any(|p| p.id == bid && p.stage == PairingStage::Completed)
    })
    .await;
    wait(&a, |s| s.connections.iter().any(|p| p.accepting)).await;
    for (service, id) in [(&a, &aid), (&b, &bid)] {
        assert_eq!(
            service.cancel_pairing(id.clone()).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
    }
    let a_final = a.subscribe();
    let b_final = b.subscribe();
    bounded(a_service.shutdown()).await.unwrap();
    bounded(b_service.shutdown()).await.unwrap();
    for (status, id) in [(a_final, aid), (b_final, bid)] {
        let snapshot = status.borrow();
        let pairing = snapshot.pairings.iter().find(|p| p.id == id).unwrap();
        assert_eq!(
            pairing.stage,
            PairingStage::Completed,
            "rejected cancellation changed a completed pairing"
        );
        assert_eq!(pairing.error, None);
    }
}

#[tokio::test]
async fn failed_pairing_persistence_never_activates_trust() {
    let (a_service, a, _a_events) = start("a").await;
    let (b_service, b, mut b_events) = start("b").await;
    let aid = a
        .start_pairing(vec![b.status().pairing_address])
        .await
        .unwrap();
    let NetworkEvent::PairingOffered { id: bid, .. } = event(&mut b_events).await else {
        panic!()
    };
    b.accept_pairing(bid.clone()).await.unwrap();
    let snapshot = wait(&b, |s| {
        s.pairings.iter().any(|p| p.id == bid && p.code.is_some())
    })
    .await;
    let code = snapshot
        .pairings
        .iter()
        .find(|p| p.id == bid)
        .unwrap()
        .code
        .clone()
        .unwrap();
    wait(&a, |s| {
        s.pairings
            .iter()
            .any(|p| p.id == aid && p.stage == PairingStage::EnteringCode)
    })
    .await;
    a.submit_pairing_code(aid, code).await.unwrap();
    let NetworkEvent::PairingVerified { id, .. } = event(&mut b_events).await else {
        panic!()
    };
    b.complete_pairing(id, false).await.unwrap();
    wait(&b, |s| {
        s.pairings
            .iter()
            .any(|p| p.id == bid && p.stage == PairingStage::Failed)
    })
    .await;
    assert!(a.status().connections.is_empty());
    assert!(b.status().connections.is_empty());
    let original_error = b
        .status()
        .pairings
        .iter()
        .find(|p| p.id == bid)
        .unwrap()
        .error;
    assert_eq!(
        b.cancel_pairing(bid.clone()).await.unwrap_err().kind(),
        ErrorKind::NotFound
    );
    let b_final = b.subscribe();
    bounded(a_service.shutdown()).await.unwrap();
    bounded(b_service.shutdown()).await.unwrap();
    let snapshot = b_final.borrow();
    let pairing = snapshot.pairings.iter().find(|p| p.id == bid).unwrap();
    assert_eq!(pairing.stage, PairingStage::Failed);
    assert_eq!(
        pairing.error, original_error,
        "rejected cancellation replaced the original failure"
    );
}

#[tokio::test]
async fn transfers_files_and_png_through_the_facade_without_clipboard_or_storage() {
    let (a_service, a, _a_events) = start("sender").await;
    let (b_service, b, mut b_events) = start("receiver").await;
    link(&a, &b).await;
    let peer = b.status().identity.id;
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let data = vec![71_u8; 700_000];
    let file = source.path().join("data.bin");
    let empty = source.path().join("empty.txt");
    std::fs::write(&file, &data).unwrap();
    std::fs::write(&empty, []).unwrap();
    std::fs::write(destination.path().join("data.bin"), b"existing").unwrap();
    let id = a
        .send(SendRequest {
            targets: vec![peer.clone()],
            content: OutgoingContent::Files(vec![file, empty]),
            queue: QueuePolicy::Append,
        })
        .await
        .unwrap();
    let NetworkEvent::IncomingOffer {
        id: incoming,
        kind,
        files,
        ..
    } = event(&mut b_events).await
    else {
        panic!("expected offer")
    };
    assert_eq!(kind, ContentKind::Files);
    assert_eq!(files.len(), 2);
    b.decide_incoming(
        incoming,
        ReceiveDecision::Accept {
            directory: Some(destination.path().into()),
        },
    )
    .await
    .unwrap();
    let NetworkEvent::ContentReady {
        id: ready,
        content: ReceivedContent::Files(paths),
        ..
    } = event(&mut b_events).await
    else {
        panic!("expected files")
    };
    assert_eq!(ready, incoming);
    assert_eq!(std::fs::read(&paths[0]).unwrap(), data);
    assert!(std::fs::read(&paths[1]).unwrap().is_empty());
    assert_eq!(
        std::fs::read(destination.path().join("data.bin")).unwrap(),
        b"existing"
    );
    b.complete_incoming(incoming, ApplicationOutcome::Saved)
        .await
        .unwrap();
    stage(&a, &id, &peer, TransferStage::Saved).await;
    let png = include_bytes!("fixtures/alpha.png").to_vec();
    let id = a
        .send(SendRequest {
            targets: vec![peer.clone()],
            content: OutgoingContent::Image(png.clone()),
            queue: QueuePolicy::Append,
        })
        .await
        .unwrap();
    let NetworkEvent::IncomingOffer {
        id: incoming,
        kind: ContentKind::Image,
        ..
    } = event(&mut b_events).await
    else {
        panic!("expected image offer")
    };
    b.decide_incoming(incoming, ReceiveDecision::Accept { directory: None })
        .await
        .unwrap();
    let NetworkEvent::ContentReady {
        content: ReceivedContent::Image(bytes),
        ..
    } = event(&mut b_events).await
    else {
        panic!("expected image")
    };
    assert_eq!(bytes, png);
    b.complete_incoming(incoming, ApplicationOutcome::Applied)
        .await
        .unwrap();
    stage(&a, &id, &peer, TransferStage::Applied).await;
    bounded(a_service.shutdown()).await.unwrap();
    bounded(b_service.shutdown()).await.unwrap();
}

#[tokio::test]
async fn cancellation_while_waiting_for_acceptance_does_not_publish_files() {
    let (a_service, a, _a_events) = start("sender").await;
    let (b_service, b, mut b_events) = start("receiver").await;
    link(&a, &b).await;
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), b"content").unwrap();
    let peer = b.status().identity.id;
    let id = a
        .send(SendRequest {
            targets: vec![peer.clone()],
            content: OutgoingContent::Files(vec![source.path().into()]),
            queue: QueuePolicy::Append,
        })
        .await
        .unwrap();
    let NetworkEvent::IncomingOffer { .. } = event(&mut b_events).await else {
        panic!()
    };
    a.cancel_transfer(id.clone()).await.unwrap();
    stage(&a, &id, &peer, TransferStage::Cancelled).await;
    bounded(a_service.shutdown()).await.unwrap();
    bounded(b_service.shutdown()).await.unwrap();
}

#[tokio::test]
async fn slow_application_is_isolated_and_a_late_receipt_resolves_uncertainty() {
    let (a_service, a, _a_events) = start("a").await;
    let (b_service, b, mut b_events) = start("slow").await;
    let (c_service, c, mut c_events) = start("fast").await;
    link(&a, &b).await;
    link(&a, &c).await;
    let bid = b.status().identity.id;
    let cid = c.status().identity.id;
    let id = a
        .send(SendRequest::text(
            vec![bid.clone(), cid.clone()],
            "one immutable send",
        ))
        .await
        .unwrap();
    let NetworkEvent::ContentReady { id: incoming, .. } = event(&mut c_events).await else {
        panic!()
    };
    c.complete_incoming(incoming, ApplicationOutcome::Applied)
        .await
        .unwrap();
    stage(&a, &id, &cid, TransferStage::Applied).await;
    stage(&a, &id, &bid, TransferStage::Unconfirmed).await;
    let NetworkEvent::ContentReady { id: incoming, .. } = event(&mut b_events).await else {
        panic!()
    };
    b.complete_incoming(incoming, ApplicationOutcome::Applied)
        .await
        .unwrap();
    stage(&a, &id, &bid, TransferStage::Applied).await;
    bounded(a_service.shutdown()).await.unwrap();
    bounded(b_service.shutdown()).await.unwrap();
    bounded(c_service.shutdown()).await.unwrap();
}

#[tokio::test]
async fn saturated_application_inbox_rejects_work_and_revocation_closes_the_peer() {
    let (a_service, a, _a_events) = start("a").await;
    let mut opts = options("b");
    opts.event_capacity = 1;
    let (b_service, b, mut b_events) = NetworkService::start(opts).await.unwrap();
    link(&a, &b).await;
    let peer = b.status().identity.id;
    let first = a
        .send(SendRequest::text(vec![peer.clone()], "first"))
        .await
        .unwrap();
    wait(&b, |s| {
        s.transfers
            .iter()
            .any(|t| t.incoming && t.stage == TransferStage::WaitingForApplication)
    })
    .await;
    let second = a
        .send(SendRequest::text(vec![peer.clone()], "second"))
        .await
        .unwrap();
    stage(&a, &second, &peer, TransferStage::Rejected).await;
    let NetworkEvent::ContentReady {
        id,
        content: ReceivedContent::Text(text),
        ..
    } = event(&mut b_events).await
    else {
        panic!()
    };
    assert_eq!(text, "first");
    b.complete_incoming(id, ApplicationOutcome::Applied)
        .await
        .unwrap();
    stage(&a, &first, &peer, TransferStage::Applied).await;
    a.revoke_peer(peer.clone()).await.unwrap();
    wait(&a, |s| s.connections.iter().all(|p| p.peer != peer)).await;
    assert_eq!(
        a.connect(peer.clone()).await.unwrap_err().kind(),
        ErrorKind::NotFound
    );
    let id = a
        .send(SendRequest::text(vec![peer.clone()], "not authorized"))
        .await
        .unwrap();
    stage(&a, &id, &peer, TransferStage::Failed).await;
    bounded(a_service.shutdown()).await.unwrap();
    bounded(b_service.shutdown()).await.unwrap();
}

#[tokio::test]
async fn shutdown_after_file_publication_reports_saved_and_waits_for_workers() {
    let (a_service, a, _a_events) = start("a").await;
    let (b_service, b, mut b_events) = start("b").await;
    link(&a, &b).await;
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), b"published content").unwrap();
    let destination = tempfile::tempdir().unwrap();
    a.send(SendRequest {
        targets: vec![b.status().identity.id],
        content: OutgoingContent::Files(vec![source.path().into()]),
        queue: QueuePolicy::Append,
    })
    .await
    .unwrap();
    let NetworkEvent::IncomingOffer { id, .. } = event(&mut b_events).await else {
        panic!()
    };
    b.decide_incoming(
        id,
        ReceiveDecision::Accept {
            directory: Some(destination.path().into()),
        },
    )
    .await
    .unwrap();
    let NetworkEvent::ContentReady {
        content: ReceivedContent::Files(paths),
        ..
    } = event(&mut b_events).await
    else {
        panic!()
    };
    let status = b.subscribe();
    bounded(b_service.shutdown()).await.unwrap();
    assert!(
        status
            .borrow()
            .transfers
            .iter()
            .any(|t| t.incoming && t.stage == TransferStage::Saved)
    );
    assert_eq!(std::fs::read(&paths[0]).unwrap(), b"published content");
    assert_eq!(std::fs::read_dir(destination.path()).unwrap().count(), 1);
    bounded(a_service.shutdown()).await.unwrap();
}

#[tokio::test]
async fn invalid_start_and_drop_have_no_live_listener_leak() {
    let mut invalid = options("invalid");
    invalid.queue_capacity = 0;
    assert_eq!(
        NetworkService::start(invalid).await.err().unwrap().kind(),
        ErrorKind::InvalidInput
    );
    let (service, network, mut events) = start("drop").await;
    let address = network.status().listen_address;
    let pairing_address = network.status().pairing_address;
    let clone = network.clone();
    let mut status = network.subscribe();
    drop(service);
    bounded(async {
        while status.borrow().state == ServiceState::Running {
            status.changed().await.unwrap();
        }
    })
    .await;
    let _listener = tokio::net::TcpListener::bind(address).await.unwrap();
    let _pairing = tokio::net::TcpListener::bind(pairing_address)
        .await
        .unwrap();
    assert_eq!(
        clone.set_accepting(true).await.unwrap_err().kind(),
        ErrorKind::Stopped
    );
    assert!(events.next_event().await.is_none());
}

#[tokio::test]
async fn late_application_results_for_images_and_saved_files_do_not_disconnect_the_peer() {
    let (a_service, a, _a_events) = start("sender").await;
    let (b_service, b, mut b_events) = start("receiver").await;
    link(&a, &b).await;
    let peer = b.status().identity.id;
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), b"saved before application").unwrap();
    let destination = tempfile::tempdir().unwrap();
    for files in [false, true] {
        let id = a
            .send(SendRequest {
                targets: vec![peer.clone()],
                content: if files {
                    OutgoingContent::Files(vec![source.path().into()])
                } else {
                    OutgoingContent::Image(include_bytes!("fixtures/alpha.png").to_vec())
                },
                queue: QueuePolicy::Append,
            })
            .await
            .unwrap();
        let NetworkEvent::IncomingOffer { id: incoming, .. } = event(&mut b_events).await else {
            panic!()
        };
        b.decide_incoming(
            incoming,
            ReceiveDecision::Accept {
                directory: files.then(|| destination.path().into()),
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            event(&mut b_events).await,
            NetworkEvent::ContentReady { .. }
        ));
        // Wait past the worker's application deadline before supplying the real outcome.
        wait(&b, |s| {
            s.transfers.iter().any(|t| {
                t.id == id
                    && t.incoming
                    && t.stage
                        == if files {
                            TransferStage::Saved
                        } else {
                            TransferStage::Unconfirmed
                        }
            })
        })
        .await;
        b.complete_incoming(incoming, ApplicationOutcome::Applied)
            .await
            .unwrap();
        stage(&a, &id, &peer, TransferStage::Applied).await;
        assert_eq!(
            b.complete_incoming(incoming, ApplicationOutcome::Applied)
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidInput
        );
        assert!(
            a.status()
                .connections
                .iter()
                .any(|p| p.peer == peer && p.accepting)
        );
        assert_eq!(a.status().state, ServiceState::Running);
    }
    bounded(a_service.shutdown()).await.unwrap();
    bounded(b_service.shutdown()).await.unwrap();
}

#[tokio::test]
async fn owner_shutdown_wakes_event_consumer_while_request_clones_remain_alive() {
    let (service, network, mut events) = start("owner").await;
    let snapshot = network.status();
    let status = network.subscribe();
    let first = network.clone();
    let second = network.clone();
    let first = bounded(tokio::spawn(async move {
        first.set_accepting(false).await.unwrap();
        first
    }))
    .await
    .unwrap();
    let (entered, started) = tokio::sync::oneshot::channel();
    let consumer = tokio::spawn(async move {
        second.local_addresses().await.unwrap();
        entered.send(()).unwrap();
        let event = events.next_event().await;
        (second, event, events)
    });
    bounded(started).await.unwrap();
    // The owner neither unwraps an Arc nor waits for either request clone to be dropped.
    bounded(service.shutdown()).await.unwrap();
    let (second, event, mut events) = bounded(consumer).await.unwrap();
    assert!(event.is_none());
    assert!(events.next_event().await.is_none());
    assert_eq!(status.borrow().state, ServiceState::Stopped);
    for handle in [&network, &first, &second] {
        assert_eq!(handle.status().state, ServiceState::Stopped);
        assert_eq!(
            handle.set_accepting(true).await.unwrap_err().kind(),
            ErrorKind::Stopped
        );
        assert_eq!(
            handle.local_addresses().await.unwrap_err().kind(),
            ErrorKind::Stopped
        );
        assert_eq!(
            handle
                .start_pairing(vec![snapshot.pairing_address])
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::Stopped
        );
        assert_eq!(
            handle
                .disconnect(snapshot.identity.id.clone())
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::Stopped
        );
    }
    let _sync = tokio::net::TcpListener::bind(snapshot.listen_address)
        .await
        .unwrap();
    let _pairing = tokio::net::TcpListener::bind(snapshot.pairing_address)
        .await
        .unwrap();
}

#[tokio::test]
async fn dropping_all_request_handles_leaves_lifetime_with_the_owner() {
    let (service, network, mut events) = start("owner only").await;
    let status = network.subscribe();
    let copy = network.clone();
    drop(network);
    drop(copy);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), events.next_event())
            .await
            .is_err()
    );
    assert_eq!(status.borrow().state, ServiceState::Running);
    bounded(service.shutdown()).await.unwrap();
    assert_eq!(status.borrow().state, ServiceState::Stopped);
    assert!(events.next_event().await.is_none());
}

#[tokio::test]
async fn dropping_the_event_receiver_rejects_new_incoming_work_without_stopping_service() {
    let (a_service, a, _a_events) = start("sender").await;
    let (b_service, b, b_events) = start("receiver").await;
    link(&a, &b).await;
    drop(b_events);
    let peer = b.status().identity.id;
    let id = a
        .send(SendRequest::text(
            vec![peer.clone()],
            "no application consumer",
        ))
        .await
        .unwrap();
    stage(&a, &id, &peer, TransferStage::Rejected).await;
    assert_eq!(b.status().state, ServiceState::Running);
    b.set_accepting(false).await.unwrap();
    bounded(a_service.shutdown()).await.unwrap();
    bounded(b_service.shutdown()).await.unwrap();
}

#[tokio::test]
async fn incoming_event_identifies_operation_and_receiver_can_cancel_before_acceptance() {
    let (a_service, a, _a_events) = start("sender").await;
    let (b_service, b, mut events) = start("receiver").await;
    link(&a, &b).await;
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), b"cancelled by receiver").unwrap();
    let peer = b.status().identity.id;
    let id = a
        .send(SendRequest {
            targets: vec![peer.clone()],
            content: OutgoingContent::Files(vec![source.path().into()]),
            queue: QueuePolicy::Append,
        })
        .await
        .unwrap();
    let NetworkEvent::IncomingOffer {
        transfer_id,
        peer: sender,
        ..
    } = event(&mut events).await
    else {
        panic!("expected offer")
    };
    assert_eq!(transfer_id, id);
    b.cancel_incoming(sender.clone(), transfer_id.clone())
        .await
        .unwrap();
    stage(&a, &id, &peer, TransferStage::Cancelled).await;
    assert_eq!(
        b.cancel_incoming(sender, transfer_id)
            .await
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidInput
    );
    a_service.shutdown().await.unwrap();
    b_service.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancelling_one_destination_keeps_shared_content_available_to_the_other() {
    let (a_service, a, _a_events) = start("sender").await;
    let (b_service, b, _b_events) = start("cancelled").await;
    let (c_service, c, mut c_events) = start("receiver").await;
    link(&a, &b).await;
    link(&a, &c).await;
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), vec![7u8; 1024 * 1024]).unwrap();
    let b_id = b.status().identity.id;
    let c_id = c.status().identity.id;
    let id = a
        .send(SendRequest {
            targets: vec![b_id.clone(), c_id.clone()],
            content: OutgoingContent::Files(vec![source.path().into()]),
            queue: QueuePolicy::Append,
        })
        .await
        .unwrap();
    a.cancel_delivery(b_id.clone(), id.clone()).await.unwrap();
    let NetworkEvent::IncomingOffer {
        id: incoming,
        transfer_id,
        ..
    } = event(&mut c_events).await
    else {
        panic!("expected offer")
    };
    assert_eq!(transfer_id, id);
    let destination = tempfile::tempdir().unwrap();
    c.decide_incoming(
        incoming,
        ReceiveDecision::Accept {
            directory: Some(destination.path().into()),
        },
    )
    .await
    .unwrap();
    let NetworkEvent::ContentReady {
        id: incoming,
        transfer_id,
        ..
    } = event(&mut c_events).await
    else {
        panic!("expected content")
    };
    assert_eq!(transfer_id, id);
    c.complete_incoming(incoming, ApplicationOutcome::Applied)
        .await
        .unwrap();
    stage(&a, &id, &b_id, TransferStage::Cancelled).await;
    stage(&a, &id, &c_id, TransferStage::Applied).await;
    a_service.shutdown().await.unwrap();
    b_service.shutdown().await.unwrap();
    c_service.shutdown().await.unwrap();
}
