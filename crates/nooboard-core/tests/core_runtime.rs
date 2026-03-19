mod support;

use std::fs;
use std::net::TcpListener;
use std::sync::Arc;

use nooboard_config::AppConfig;
use nooboard_core::{
    ClipboardHistoryDirection, ClipboardRecordSource, ListClipboardHistoryRequest, NetworkStatus,
    SendFilesRequest, SessionTarget, TransferOutcome,
};
use tokio::time::Duration;

use support::{
    TestError, accept_incoming_transfer, committed_clipboard_event, connect_core_pair,
    incoming_transfer_ticket, new_core, new_core_pair, restart_core, transfer_completed_event,
    wait_for_event, wait_for_sessions, wait_for_snapshot, wait_for_state_update,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn submit_text_and_adopt_round_trip() -> Result<(), TestError> {
    let env = new_core()?;
    let core = &env.core;
    let backend = &env.backend;
    let mut state_subscription = core.subscribe_state().await?;
    let mut event_subscription = core.subscribe_events().await?;
    let initial_snapshot = state_subscription.latest().clone();

    let event_id = core.submit_text("hello".to_string()).await?;
    let (committed_event_id, source) = wait_for_event(
        &mut event_subscription,
        Duration::from_secs(2),
        committed_clipboard_event,
    )
    .await?;
    assert_eq!(committed_event_id, event_id);
    assert_eq!(source, ClipboardRecordSource::UserSubmit);

    let updated_snapshot = wait_for_state_update(
        &mut state_subscription,
        Duration::from_secs(2),
        |snapshot| snapshot.clipboard.latest_committed_event_id == Some(event_id),
    )
    .await?;
    assert!(updated_snapshot.revision > initial_snapshot.revision);

    let record = core.get_clipboard_record(event_id).await?;
    assert_eq!(record.content, "hello");
    assert_eq!(record.source, ClipboardRecordSource::UserSubmit);

    let history = core
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            direction: ClipboardHistoryDirection::Older,
            anchor: None,
        })
        .await?;
    assert_eq!(history.records.len(), 1);
    assert!(!history.has_more);

    core.adopt_clipboard_record(event_id).await?;
    assert_eq!(backend.last_written().as_deref(), Some("hello"));

    let history_after_adopt = core
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            direction: ClipboardHistoryDirection::Older,
            anchor: None,
        })
        .await?;
    assert_eq!(history_after_adopt.records.len(), 1);

    core.shutdown().await?;
    Ok(())
}

#[test]
fn launch_does_not_require_callers_tokio_runtime() -> Result<(), TestError> {
    let env = new_core()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(env.core.shutdown())?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_capture_enabled_persists_and_restarts_watch_on_launch() -> Result<(), TestError> {
    let env = new_core()?;
    let core = &env.core;

    core.set_local_capture_enabled(true).await?;
    let mut event_subscription = core.subscribe_events().await?;
    env.backend.emit_watch_text("captured-locally");

    let (event_id, source) = wait_for_event(
        &mut event_subscription,
        Duration::from_secs(2),
        committed_clipboard_event,
    )
    .await?;
    assert_eq!(source, ClipboardRecordSource::LocalCapture);
    assert_eq!(
        core.get_clipboard_record(event_id).await?.content,
        "captured-locally"
    );

    core.shutdown().await?;

    let restarted_backend = Arc::new(support::MockClipboardBackend::default());
    let restarted = restart_core(&env.config_path, restarted_backend.clone())?;
    assert!(
        restarted
            .snapshot()
            .await?
            .settings
            .clipboard
            .local_capture_enabled
    );

    let mut restarted_events = restarted.subscribe_events().await?;
    restarted_backend.emit_watch_text("after-restart-capture");
    let (_, restarted_source) = wait_for_event(
        &mut restarted_events,
        Duration::from_secs(2),
        committed_clipboard_event,
    )
    .await?;
    assert_eq!(restarted_source, ClipboardRecordSource::LocalCapture);

    restarted.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn start_network_failure_refreshes_snapshot_to_error_state() -> Result<(), TestError> {
    let env = new_core()?;
    let mut state_subscription = env.core.subscribe_state().await?;
    let initial_snapshot = state_subscription.latest().clone();
    let _occupied = TcpListener::bind(("0.0.0.0", env.listen_port))?;

    let error = env.core.start_network().await.expect_err("start must fail");
    assert!(matches!(error, nooboard_core::CoreError::Network(_)));

    let snapshot = wait_for_state_update(
        &mut state_subscription,
        Duration::from_secs(2),
        |snapshot| matches!(snapshot.network.status, NetworkStatus::Error(_)),
    )
    .await?;
    assert!(snapshot.revision > initial_snapshot.revision);

    env.core.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_network_rebuild_refreshes_snapshot_to_persisted_config() -> Result<(), TestError> {
    let env = new_core()?;
    env.core.start_network().await?;
    let _ = wait_for_snapshot(&env.core, Duration::from_secs(2), |snapshot| {
        matches!(snapshot.network.status, NetworkStatus::Running)
    })
    .await?;

    let occupied = TcpListener::bind(("0.0.0.0", 0))?;
    let occupied_port = occupied.local_addr()?.port();
    assert_ne!(occupied_port, env.listen_port);

    let mut state_subscription = env.core.subscribe_state().await?;
    let error = env
        .core
        .set_network_listen_port(occupied_port)
        .await
        .expect_err("rebuild must fail");
    assert!(matches!(error, nooboard_core::CoreError::Network(_)));

    let snapshot = wait_for_state_update(
        &mut state_subscription,
        Duration::from_secs(2),
        |snapshot| {
            snapshot.settings.network.listen_port == occupied_port
                && matches!(snapshot.network.status, NetworkStatus::Error(_))
        },
    )
    .await?;
    assert_eq!(snapshot.settings.network.listen_port, occupied_port);

    let persisted = AppConfig::load(&env.config_path)?;
    assert_eq!(persisted.network.listen_port, occupied_port);

    env.core.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_connect_and_submit_text_commits_remote_sync() -> Result<(), TestError> {
    let (env_a, env_b) = new_core_pair()?;
    connect_core_pair(&env_a.core, &env_b.core, env_b.listen_port).await?;

    let mut events_b = env_b.core.subscribe_events().await?;
    let event_id = env_a.core.submit_text("remote-text".to_string()).await?;
    let (received_event_id, source) = wait_for_event(
        &mut events_b,
        Duration::from_secs(10),
        committed_clipboard_event,
    )
    .await?;
    assert_eq!(received_event_id, event_id);
    assert_eq!(source, ClipboardRecordSource::RemoteSync);

    let sender_identity = env_a.core.snapshot().await?.identity;
    let record_b = env_b.core.get_clipboard_record(event_id).await?;
    assert_eq!(record_b.content, "remote-text");
    assert_eq!(record_b.source, ClipboardRecordSource::RemoteSync);
    assert_eq!(record_b.origin_noob_id, sender_identity.noob_id);
    assert_eq!(record_b.origin_device_id, sender_identity.device_id);

    env_a.core.shutdown().await?;
    env_b.core.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disconnect_session_after_listing_runtime_sessions_succeeds() -> Result<(), TestError> {
    let (env_a, env_b) = new_core_pair()?;
    connect_core_pair(&env_a.core, &env_b.core, env_b.listen_port).await?;

    let sessions = env_a.core.list_sessions().await?;
    assert_eq!(sessions.len(), 1);

    env_a.core.disconnect_session(sessions[0].id).await?;
    let _ = wait_for_sessions(&env_a.core, 0).await?;
    let _ = wait_for_sessions(&env_b.core, 0).await?;

    env_a.core.shutdown().await?;
    env_b.core.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_files_updates_transfer_snapshot_and_writes_downloaded_file() -> Result<(), TestError>
{
    let (env_a, env_b) = new_core_pair()?;
    connect_core_pair(&env_a.core, &env_b.core, env_b.listen_port).await?;

    let mut events_a = env_a.core.subscribe_events().await?;
    let mut events_b = env_b.core.subscribe_events().await?;
    let source_path = env_a.dir.path().join("demo.txt");
    fs::write(&source_path, b"file-body")?;

    let tickets = env_a
        .core
        .send_files(SendFilesRequest {
            files: vec![source_path.clone()],
            target: SessionTarget::AllConnected,
        })
        .await?;
    assert_eq!(tickets.len(), 1);

    let offered_ticket = wait_for_event(
        &mut events_b,
        Duration::from_secs(10),
        incoming_transfer_ticket,
    )
    .await?;
    accept_incoming_transfer(&env_b.core, offered_ticket).await?;

    let (completed_ticket_b, _) = wait_for_event(
        &mut events_b,
        Duration::from_secs(10),
        transfer_completed_event,
    )
    .await?;
    assert_eq!(completed_ticket_b, offered_ticket);

    let (completed_ticket_a, _) = wait_for_event(
        &mut events_a,
        Duration::from_secs(10),
        transfer_completed_event,
    )
    .await?;
    assert_eq!(completed_ticket_a, tickets[0]);

    let receiver_snapshot = wait_for_snapshot(&env_b.core, Duration::from_secs(10), |snapshot| {
        snapshot
            .network
            .transfers
            .recent_completed
            .iter()
            .any(|transfer| transfer.ticket == offered_ticket)
    })
    .await?;
    assert!(
        receiver_snapshot
            .network
            .transfers
            .recent_completed
            .iter()
            .any(|transfer| transfer.ticket == offered_ticket)
    );
    assert!(
        receiver_snapshot
            .network
            .transfers
            .incoming_pending
            .iter()
            .all(|transfer| transfer.ticket != offered_ticket)
    );

    env_a.core.shutdown().await?;
    env_b.core.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_transfer_immediately_after_send_files_succeeds() -> Result<(), TestError> {
    let (env_a, env_b) = new_core_pair()?;
    connect_core_pair(&env_a.core, &env_b.core, env_b.listen_port).await?;

    let mut events_a = env_a.core.subscribe_events().await?;
    let source_path = env_a.dir.path().join("queued.txt");
    fs::write(&source_path, b"file-body")?;

    let tickets = env_a
        .core
        .send_files(SendFilesRequest {
            files: vec![source_path],
            target: SessionTarget::AllConnected,
        })
        .await?;
    assert_eq!(tickets.len(), 1);

    env_a.core.cancel_transfer(tickets[0]).await?;

    let (completed_ticket, outcome) = wait_for_event(
        &mut events_a,
        Duration::from_secs(10),
        transfer_completed_event,
    )
    .await?;
    assert_eq!(completed_ticket, tickets[0]);
    assert_eq!(outcome, TransferOutcome::Cancelled);

    env_a.core.shutdown().await?;
    env_b.core.shutdown().await?;
    Ok(())
}
