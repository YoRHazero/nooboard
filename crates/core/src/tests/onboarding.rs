use super::*;
use crate::PairingStage;
fn session(app: &App) -> crate::PairingSession {
    app.snapshot().onboarding.session.unwrap()
}
async fn request_pair(a: &App, b: &App) {
    a.begin_pairing(b.snapshot().onboarding.pairing_address, None)
        .await
        .unwrap();
    wait_for(|| b.snapshot().onboarding.session.is_some()).await;
    b.accept_pairing(session(b).id).await.unwrap();
    wait_for(|| session(a).stage == PairingStage::EnteringCode && session(b).code.is_some()).await;
}
#[tokio::test]
async fn one_time_code_pairs_both_devices_then_uses_existing_tls_without_auto_send() {
    let ca = FakeClipboard::new();
    let cb = FakeClipboard::new();
    let a = app(&ca).await;
    let b = app(&cb).await;
    request_pair(&a, &b).await;
    assert!(a.status().peers.is_empty() && b.status().peers.is_empty());
    assert!(session(&a).code.is_none());
    a.submit_pairing_code(session(&a).id, session(&b).code.unwrap())
        .await
        .unwrap();
    wait_for(|| {
        session(&a).stage == PairingStage::Completed && session(&b).stage == PairingStage::Completed
    })
    .await;
    wait_for(|| ready(&a, &b) && ready(&b, &a)).await;
    assert!(session(&a).code.is_none() && session(&b).code.is_none());
    assert_eq!(a.status().peers[0].noob_id, b.status().noob_id);
    assert!(!a.status().peers[0].settings.auto_send && !b.status().peers[0].settings.auto_send);
    ca.text("配对码完成后使用原有 TLS");
    let id = a.send_to(vec![b.status().noob_id]).await.unwrap();
    wait_for(|| outcome(&a, &id, &b.status().noob_id) == Some(DeliveryState::Applied)).await;
    assert_eq!(
        cb.current(),
        ReadState::Ready(Payload::Text("配对码完成后使用原有 TLS".into()))
    );
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}
#[tokio::test]
async fn incorrect_codes_cannot_persist_trust_and_three_attempts_close_the_session() {
    let a = app(&FakeClipboard::new()).await;
    let b = app(&FakeClipboard::new()).await;
    request_pair(&a, &b).await;
    let correct = session(&b).code.unwrap();
    let wrong = if correct == "00000000" {
        "11111111"
    } else {
        "00000000"
    };
    for left in (0..3).rev() {
        a.submit_pairing_code(session(&a).id, wrong.into())
            .await
            .unwrap();
        wait_for(|| {
            if left == 0 {
                session(&a).stage == PairingStage::Failed
            } else {
                session(&a).stage == PairingStage::EnteringCode && session(&a).attempts_left == left
            }
        })
        .await;
        assert!(a.status().peers.is_empty() && b.status().peers.is_empty());
    }
    wait_for(|| session(&b).stage == PairingStage::Failed).await;
    assert!(session(&b).code.is_none());
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}
#[tokio::test]
async fn cancellation_rejects_late_actions_and_discovery_identity_is_not_trusted() {
    let a = app(&FakeClipboard::new()).await;
    let b = app(&FakeClipboard::new()).await;
    a.begin_pairing(
        b.snapshot().onboarding.pairing_address,
        Some("a".repeat(64)),
    )
    .await
    .unwrap();
    wait_for(|| b.snapshot().onboarding.session.is_some()).await;
    b.accept_pairing(session(&b).id).await.unwrap();
    wait_for(|| session(&a).stage == PairingStage::Failed).await;
    assert!(a.status().peers.is_empty() && b.status().peers.is_empty());
    let old = session(&b).id;
    b.dismiss_pairing(old.clone()).await.unwrap();
    assert!(b.accept_pairing(old).await.is_err());
    assert!(b.snapshot().onboarding.session.is_none());
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}
#[tokio::test]
async fn another_pairing_request_does_not_interrupt_existing_request_or_sync() {
    let ca = FakeClipboard::new();
    let cb = FakeClipboard::new();
    let a = app(&ca).await;
    let b = app(&cb).await;
    let c = app(&FakeClipboard::new()).await;
    pair(&a, &b).await;
    request_pair(&c, &b).await;
    let request = session(&b).id;
    assert!(matches!(
        b.begin_pairing(a.snapshot().onboarding.pairing_address, None)
            .await,
        Err(Error::Busy)
    ));
    ca.text("已有连接继续工作");
    a.send_current().await.unwrap();
    wait_for(|| cb.current() == ReadState::Ready(Payload::Text("已有连接继续工作".into()))).await;
    assert_eq!(session(&b).id, request);
    c.dismiss_pairing(session(&c).id).await.unwrap();
    b.dismiss_pairing(request).await.unwrap();
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
    c.shutdown().await.unwrap();
}

#[tokio::test]
async fn refreshing_discovery_preserves_pending_pairing_and_existing_sync() {
    let ca = FakeClipboard::new();
    let cb = FakeClipboard::new();
    let a = app(&ca).await;
    let b = app(&cb).await;
    let c = app(&FakeClipboard::new()).await;
    pair(&a, &b).await;
    request_pair(&c, &b).await;
    let request = session(&b);
    let pairing_address = b.snapshot().onboarding.pairing_address;
    b.refresh_discovery().await.unwrap();
    // Exercise a second refresh after the rate limit, while a code is pending.
    tokio::time::sleep(Duration::from_millis(3100)).await;
    b.refresh_discovery().await.unwrap();
    assert!(b.snapshot().onboarding.discovery_error.is_none());
    assert_eq!(b.snapshot().onboarding.pairing_address, pairing_address);
    assert_eq!(session(&b).id, request.id);
    assert_eq!(session(&b).code, request.code);
    ca.text("刷新发现时原有连接继续工作");
    a.send_current().await.unwrap();
    wait_for(|| {
        cb.current() == ReadState::Ready(Payload::Text("刷新发现时原有连接继续工作".into()))
    })
    .await;
    c.submit_pairing_code(session(&c).id, request.code.unwrap())
        .await
        .unwrap();
    wait_for(|| {
        session(&b).stage == PairingStage::Completed && session(&c).stage == PairingStage::Completed
    })
    .await;
    for app in [a, b, c] {
        app.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn dismissal_notifies_a_peer_waiting_for_local_input_without_waiting_for_timeout() {
    let a = app(&FakeClipboard::new()).await;
    let b = app(&FakeClipboard::new()).await;
    request_pair(&a, &b).await;
    b.dismiss_pairing(session(&b).id).await.unwrap();
    wait_for(|| session(&a).stage == PairingStage::Failed).await;
    assert!(a.status().peers.is_empty() && b.status().peers.is_empty());
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_correct_retry_preserves_the_original_expiry_and_completes_pairing() {
    let a = app(&FakeClipboard::new()).await;
    let b = app(&FakeClipboard::new()).await;
    request_pair(&a, &b).await;
    let correct = session(&b).code.unwrap();
    let expiry = session(&b).expires_at_ms;
    let wrong = if correct == "00000000" {
        "11111111"
    } else {
        "00000000"
    };
    a.submit_pairing_code(session(&a).id, wrong.into())
        .await
        .unwrap();
    wait_for(|| session(&a).stage == PairingStage::EnteringCode && session(&a).attempts_left == 2)
        .await;
    assert_eq!(session(&b).expires_at_ms, expiry);
    assert_eq!(session(&b).code.as_deref(), Some(correct.as_str()));
    a.submit_pairing_code(session(&a).id, correct)
        .await
        .unwrap();
    wait_for(|| {
        session(&a).stage == PairingStage::Completed && session(&b).stage == PairingStage::Completed
    })
    .await;
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}
