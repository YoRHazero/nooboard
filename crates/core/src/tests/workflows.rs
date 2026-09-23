use super::*;
#[tokio::test]
async fn manual_text_preserves_bytes_and_reports_real_application_failure() {
    let a = Clipboard::new();
    let b = Clipboard::new();
    let (sa, aa) = start(&a).await;
    let (sb, ab) = start(&b).await;
    pair(&aa, &ab).await;
    let text = "  中文🦀\r\n exact text ";
    a.copy(Payload::Text(text.into()));
    let id = aa.send_current().await.unwrap();
    delivered(&aa, &id, DeliveryState::Applied).await;
    assert_eq!(b.text().as_deref(), Some(text));
    wait(&ab, |s| s.history_revision > 0).await;
    let rows = ab.history(String::new(), 20, 0).await.unwrap();
    assert_eq!(rows[0].text, text);
    assert_eq!(rows[0].source, aa.status().noob_id);
    b.fail.store(true, Ordering::SeqCst);
    a.copy(Payload::Text("cannot apply".into()));
    let id = aa.send_current().await.unwrap();
    delivered(&aa, &id, DeliveryState::Rejected).await;
    assert_eq!(b.text().as_deref(), Some(text));
    sa.shutdown().await.unwrap();
    sb.shutdown().await.unwrap();
}
#[tokio::test]
async fn automatic_routing_never_echoes_remote_or_history_writes_and_resume_does_not_replay() {
    let a = Clipboard::new();
    let b = Clipboard::new();
    let (sa, aa) = start(&a).await;
    let (sb, ab) = start(&b).await;
    pair(&aa, &ab).await;
    for (app, other) in [(&aa, &ab), (&ab, &aa)] {
        app.configure_peer(
            other.status().noob_id,
            PeerSettings {
                address: None,
                auto_send: true,
            },
        )
        .await
        .unwrap();
        app.set_settings(Settings {
            mode: Mode::Automatic,
            ..app.status().settings
        })
        .await
        .unwrap();
        wait(app, |s| {
            s.status.effective_revision == s.status.configuration_revision
        })
        .await;
    }
    a.copy(Payload::Text("automatic".into()));
    wait(&ab, |s| s.current.text.as_deref() == Some("automatic")).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(ab.status().transfers.is_empty());
    wait(&ab, |s| s.history_revision > 0).await;
    let id = ab.history(String::new(), 20, 0).await.unwrap()[0].id;
    ab.copy_history(id).await.unwrap();
    assert!(ab.status().transfers.is_empty());
    aa.set_settings(Settings {
        paused: true,
        ..aa.status().settings
    })
    .await
    .unwrap();
    wait(&aa, |s| {
        s.status.effective_revision == s.status.configuration_revision
    })
    .await;
    a.copy(Payload::Text("paused".into()));
    wait(&aa, |s| s.current.text.as_deref() == Some("paused")).await;
    aa.set_settings(Settings {
        paused: false,
        ..aa.status().settings
    })
    .await
    .unwrap();
    wait(&aa, |s| {
        s.status.effective_revision == s.status.configuration_revision
    })
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(b.text().as_deref(), Some("automatic"));
    a.copy(Payload::Text("new copy".into()));
    wait(&ab, |s| s.current.text.as_deref() == Some("new copy")).await;
    sa.shutdown().await.unwrap();
    sb.shutdown().await.unwrap();
}
#[tokio::test]
async fn images_and_files_share_application_path_and_saved_files_survive_clipboard_failure() {
    let a = Clipboard::new();
    let b = Clipboard::new();
    let (sa, aa) = start(&a).await;
    let (sb, ab) = start(&b).await;
    pair(&aa, &ab).await;
    let directory = tempfile::tempdir().unwrap();
    ab.set_settings(Settings {
        receive_directory: Some(directory.path().into()),
        ..ab.status().settings
    })
    .await
    .unwrap();
    a.copy(Payload::Image(
        nooboard_clipboard::ImageData::new(
            nooboard_clipboard::ImageEncoding::Png,
            include_bytes!("fixtures/alpha.png").to_vec(),
        )
        .unwrap(),
    ));
    let id = aa.send_current().await.unwrap();
    wait(&aa, |s| {
        s.content_transfers
            .iter()
            .any(|t| t.id == id && t.stage == ContentStage::Completed)
    })
    .await;
    assert!(matches!(
        b.state.borrow().as_ref().unwrap().content,
        ReadState::Ready(Payload::Image(_))
    ));
    assert!(ab.history(String::new(), 20, 0).await.unwrap().is_empty());
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), b"file body").unwrap();
    b.fail.store(true, Ordering::SeqCst);
    aa.send_files(vec![source.path().into()]).await.unwrap();
    wait(&ab, |s| {
        s.content_transfers
            .iter()
            .any(|t| t.stage == ContentStage::Saved)
    })
    .await;
    let received = ab
        .snapshot()
        .content_transfers
        .into_iter()
        .find(|t| t.stage == ContentStage::Saved)
        .unwrap();
    assert_eq!(
        std::fs::read(&received.saved_paths[0]).unwrap(),
        b"file body"
    );
    // Explicit copy remains possible even when the initial native write failed.
    b.fail.store(false, Ordering::SeqCst);
    ab.copy_received(received.key).await.unwrap();
    assert!(matches!(
        b.state.borrow().as_ref().unwrap().content,
        ReadState::Ready(Payload::Files(_))
    ));
    sa.shutdown().await.unwrap();
    sb.shutdown().await.unwrap();
}
#[tokio::test]
async fn real_pairing_persists_verified_identity_and_dismiss_does_not_revoke_success() {
    let a = Clipboard::new();
    let b = Clipboard::new();
    let (sa, aa) = start(&a).await;
    let (sb, ab) = start(&b).await;
    wait(&ab, |s| !s.onboarding.pairing_address.is_empty()).await;
    aa.begin_pairing(
        ab.snapshot().onboarding.pairing_address,
        Some(ab.status().noob_id),
    )
    .await
    .unwrap();
    wait(&ab, |s| {
        s.onboarding
            .session
            .as_ref()
            .is_some_and(|p| p.stage == PairingStage::AwaitingApproval)
    })
    .await;
    let bid = ab.snapshot().onboarding.session.unwrap().id;
    ab.accept_pairing(bid.clone()).await.unwrap();
    wait(&ab, |s| {
        s.onboarding
            .session
            .as_ref()
            .is_some_and(|p| p.code.is_some())
    })
    .await;
    wait(&aa, |s| {
        s.onboarding
            .session
            .as_ref()
            .is_some_and(|p| p.stage == PairingStage::EnteringCode)
    })
    .await;
    let aid = aa.snapshot().onboarding.session.unwrap().id;
    let code = ab.snapshot().onboarding.session.unwrap().code.unwrap();
    aa.submit_pairing_code(aid.clone(), code).await.unwrap();
    for app in [&aa, &ab] {
        wait(app, |s| {
            s.onboarding
                .session
                .as_ref()
                .is_some_and(|p| p.stage == PairingStage::Completed)
                && s.status.peers.len() == 1
        })
        .await;
        assert!(!app.status().peers[0].settings.auto_send);
    }
    aa.dismiss_pairing(aid).await.unwrap();
    assert_eq!(aa.status().peers.len(), 1);
    aa.unpair(ab.status().noob_id).await.unwrap();
    wait(&aa, |s| s.status.peers.is_empty()).await;
    sa.shutdown().await.unwrap();
    sb.shutdown().await.unwrap();
}
