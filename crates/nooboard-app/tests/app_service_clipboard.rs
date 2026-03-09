mod support;

use std::sync::Arc;

use nooboard_app::{
    AppError, ClipboardBroadcastTargets, ClipboardRecordSource, ClipboardSettingsPatch,
    DesktopAppService, EventId, ListClipboardHistoryRequest, NoobId, RebroadcastClipboardRequest,
    SettingsPatch, SubmitTextRequest,
};

use support::{
    TestError, connect_service_fanout, connect_service_pair, new_service, new_service_fanout,
    new_service_pair, recv_clipboard_committed, restart_service, wait_for_service_state,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clipboard_committed_event_only_follows_successful_commit() -> Result<(), TestError> {
    let env = new_service()?;
    let service = &env.service;
    let mut event_subscription = service.subscribe_events().await?;

    let event_id = service
        .submit_text(SubmitTextRequest {
            content: "short".to_string(),
        })
        .await?;
    let (committed_event_id, _) = recv_clipboard_committed(&mut event_subscription).await?;
    assert_eq!(committed_event_id, event_id);

    let record = service.get_clipboard_record(event_id).await?;
    assert_eq!(record.content, "short");
    assert_eq!(
        service
            .get_state()
            .await?
            .clipboard
            .latest_committed_event_id,
        Some(event_id)
    );

    let too_large = "x".repeat(65);
    let error = service
        .submit_text(SubmitTextRequest { content: too_large })
        .await
        .expect_err("oversized content must fail");
    assert!(matches!(error, AppError::TextTooLarge { .. }));

    let maybe_event = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        event_subscription.recv(),
    )
    .await;
    assert!(
        maybe_event.is_err(),
        "failed commit must not emit ClipboardCommitted"
    );

    service.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adopt_clipboard_record_does_not_create_new_record() -> Result<(), TestError> {
    let env = new_service()?;
    let service = &env.service;
    let backend = &env.backend;

    let event_id = service
        .submit_text(SubmitTextRequest {
            content: "adopt-me".to_string(),
        })
        .await?;

    let before = service
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(before.records.len(), 1);

    service.adopt_clipboard_record(event_id).await?;
    assert_eq!(backend.last_written().as_deref(), Some("adopt-me"));

    let after = service
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(after.records.len(), 1);
    assert_eq!(after.records[0].event_id, event_id);

    service.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_capture_source_persists_across_restart() -> Result<(), TestError> {
    let env = new_service()?;
    let service = &env.service;
    let mut events = service.subscribe_events().await?;

    service
        .patch_settings(SettingsPatch::Clipboard(
            ClipboardSettingsPatch::SetLocalCaptureEnabled(true),
        ))
        .await?;

    env.backend.emit_watch_text("captured-locally");
    let (event_id, source) = recv_clipboard_committed(&mut events).await?;
    assert_eq!(source, ClipboardRecordSource::LocalCapture);

    service.shutdown().await?;

    let restarted_backend = Arc::new(support::MockClipboardBackend::default());
    let restarted = restart_service(&env.config_path, restarted_backend)?;
    let record = restarted.get_clipboard_record(event_id).await?;
    assert_eq!(record.source, ClipboardRecordSource::LocalCapture);

    let history = restarted
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(history.records.len(), 1);
    assert_eq!(
        history.records[0].source,
        ClipboardRecordSource::LocalCapture
    );

    restarted.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rebroadcast_clipboard_record_sends_committed_record_without_new_local_record()
-> Result<(), TestError> {
    let (env_a, env_b) = new_service_pair()?;
    let service_a = &env_a.service;
    let service_b = &env_b.service;
    let (noob_id_a, noob_id_b) = connect_service_pair(service_a, service_b).await?;
    let mut receiver_events = service_b.subscribe_events().await?;

    let event_id = service_a
        .submit_text(SubmitTextRequest {
            content: "rebroadcast-me".to_string(),
        })
        .await?;

    service_a
        .rebroadcast_clipboard_record(RebroadcastClipboardRequest {
            event_id,
            targets: ClipboardBroadcastTargets::Nodes(vec![noob_id_b.clone()]),
        })
        .await?;

    let (received_event_id, source) = recv_clipboard_committed(&mut receiver_events).await?;
    assert_eq!(received_event_id, event_id);
    assert_eq!(source, ClipboardRecordSource::RemoteSync);

    let receiver_record = service_b.get_clipboard_record(event_id).await?;
    assert_eq!(receiver_record.source, ClipboardRecordSource::RemoteSync);
    assert_eq!(receiver_record.content, "rebroadcast-me");
    assert_eq!(receiver_record.origin_noob_id, noob_id_a);

    let sender_history = service_a
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(sender_history.records.len(), 1);
    assert_eq!(sender_history.records[0].event_id, event_id);

    let receiver_state =
        wait_for_service_state(service_b, std::time::Duration::from_secs(10), |state| {
            state.clipboard.latest_committed_event_id == Some(event_id)
        })
        .await?;
    assert_eq!(
        receiver_state.clipboard.latest_committed_event_id,
        Some(event_id)
    );

    service_a.shutdown().await?;
    service_b.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rebroadcast_clipboard_record_all_connected_reaches_every_connected_peer()
-> Result<(), TestError> {
    let (env_a, env_b, env_c) = new_service_fanout()?;
    let service_a = &env_a.service;
    let service_b = &env_b.service;
    let service_c = &env_c.service;
    let (noob_id_a, _, _) = connect_service_fanout(service_a, service_b, service_c).await?;
    let mut events_b = service_b.subscribe_events().await?;
    let mut events_c = service_c.subscribe_events().await?;

    let event_id = service_a
        .submit_text(SubmitTextRequest {
            content: "fanout-rebroadcast".to_string(),
        })
        .await?;

    service_a
        .rebroadcast_clipboard_record(RebroadcastClipboardRequest {
            event_id,
            targets: ClipboardBroadcastTargets::AllConnected,
        })
        .await?;

    let (event_id_b, source_b) = recv_clipboard_committed(&mut events_b).await?;
    let (event_id_c, source_c) = recv_clipboard_committed(&mut events_c).await?;
    assert_eq!(event_id_b, event_id);
    assert_eq!(event_id_c, event_id);
    assert_eq!(source_b, ClipboardRecordSource::RemoteSync);
    assert_eq!(source_c, ClipboardRecordSource::RemoteSync);

    let record_b = service_b.get_clipboard_record(event_id).await?;
    let record_c = service_c.get_clipboard_record(event_id).await?;
    assert_eq!(record_b.origin_noob_id, noob_id_a);
    assert_eq!(record_c.origin_noob_id, noob_id_a);
    assert_eq!(record_b.content, "fanout-rebroadcast");
    assert_eq!(record_c.content, "fanout-rebroadcast");

    let history_b = service_b
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    let history_c = service_c
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(history_b.records.len(), 1);
    assert_eq!(history_c.records.len(), 1);

    service_a.shutdown().await?;
    service_b.shutdown().await?;
    service_c.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rebroadcast_clipboard_record_returns_structured_errors() -> Result<(), TestError> {
    let env = new_service()?;
    let service = &env.service;

    let existing_event_id = service
        .submit_text(SubmitTextRequest {
            content: "rebroadcast-local".to_string(),
        })
        .await?;

    let missing_event_error = service
        .rebroadcast_clipboard_record(RebroadcastClipboardRequest {
            event_id: EventId::new(),
            targets: ClipboardBroadcastTargets::AllConnected,
        })
        .await
        .expect_err("missing event must fail");
    assert!(matches!(
        missing_event_error,
        AppError::EventNotFound { .. }
    ));

    let offline_error = service
        .rebroadcast_clipboard_record(RebroadcastClipboardRequest {
            event_id: existing_event_id,
            targets: ClipboardBroadcastTargets::Nodes(vec![NoobId::new("offline-node")]),
        })
        .await
        .expect_err("offline target must fail");
    assert!(matches!(
        offline_error,
        AppError::PeerNotConnected { ref peer_noob_id } if peer_noob_id == "offline-node"
    ));

    let history = service
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(history.records.len(), 1);

    service.shutdown().await?;
    Ok(())
}
