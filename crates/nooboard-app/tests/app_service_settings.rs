mod support;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use nooboard_app::{
    AppError, ClipboardRecordSource, ClipboardSettingsPatch, DesktopAppService,
    IncomingTransferDecision, IncomingTransferDisposition, ListClipboardHistoryRequest,
    NetworkSettingsPatch, NoobId, SendFileItem, SendFilesRequest, SettingsPatch,
    StorageSettingsPatch, SubmitTextRequest, SyncActualStatus, SyncDesiredState,
    TransferSettingsPatch,
};
use tokio::time::{Duration, timeout};

use support::{
    MockClipboardBackend, TestError, connect_service_pair, new_service, new_service_pair,
    recv_clipboard_committed, restart_service, wait_for_event, wait_for_service_state,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_capture_enabled_persists_across_restart_and_restarts_watch() -> Result<(), TestError>
{
    let env = new_service()?;
    let service = &env.service;

    service
        .patch_settings(SettingsPatch::Clipboard(
            ClipboardSettingsPatch::SetLocalCaptureEnabled(true),
        ))
        .await?;
    assert!(
        service
            .get_state()
            .await?
            .settings
            .clipboard
            .local_capture_enabled
    );

    service.shutdown().await?;

    let restarted_backend = Arc::new(MockClipboardBackend::default());
    let restarted = restart_service(&env.config_path, restarted_backend.clone())?;
    assert!(
        restarted
            .get_state()
            .await?
            .settings
            .clipboard
            .local_capture_enabled
    );

    let mut events = restarted.subscribe_events().await?;
    restarted_backend.emit_watch_text("after-restart-capture");
    let (event_id, source) = recv_clipboard_committed(&mut events).await?;
    assert_eq!(source, ClipboardRecordSource::LocalCapture);
    assert_eq!(
        restarted.get_clipboard_record(event_id).await?.content,
        "after-restart-capture"
    );

    restarted.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn download_dir_persists_across_restart_and_controls_download_target() -> Result<(), TestError>
{
    let (env_a, env_b) = new_service_pair()?;
    let service_a = &env_a.service;
    let service_b = &env_b.service;
    let custom_download_dir = env_b.dir.path().join("downloads-custom");

    service_b
        .patch_settings(SettingsPatch::Transfers(
            TransferSettingsPatch::SetDownloadDir(custom_download_dir.clone()),
        ))
        .await?;
    assert_eq!(
        service_b.get_state().await?.settings.transfers.download_dir,
        custom_download_dir
    );

    service_b.shutdown().await?;

    let restarted_backend = Arc::new(MockClipboardBackend::default());
    let restarted_b = restart_service(&env_b.config_path, restarted_backend)?;
    assert_eq!(
        restarted_b
            .get_state()
            .await?
            .settings
            .transfers
            .download_dir,
        custom_download_dir
    );

    let (noob_id_a, noob_id_b) = connect_service_pair(service_a, &restarted_b).await?;
    let mut sender_events = service_a.subscribe_events().await?;
    let mut receiver_events = restarted_b.subscribe_events().await?;

    let source_file = env_a.dir.path().join("settings-download.txt");
    std::fs::write(&source_file, b"settings-download")?;

    let sender_transfer_id = service_a
        .send_files(nooboard_app::SendFilesRequest {
            targets: vec![noob_id_b.clone()],
            files: vec![nooboard_app::SendFileItem { path: source_file }],
        })
        .await?
        .into_iter()
        .next()
        .expect("must create transfer");

    let receiver_pending = wait_for_service_state(&restarted_b, Duration::from_secs(10), |state| {
        state
            .transfers
            .incoming_pending
            .iter()
            .any(|pending| pending.peer_noob_id == noob_id_a)
    })
    .await?;
    let receiver_transfer_id = receiver_pending
        .transfers
        .incoming_pending
        .iter()
        .find(|pending| pending.peer_noob_id == noob_id_a)
        .expect("receiver must expose pending transfer")
        .transfer_id
        .clone();

    restarted_b
        .decide_incoming_transfer(IncomingTransferDecision {
            transfer_id: receiver_transfer_id.clone(),
            decision: IncomingTransferDisposition::Accept,
        })
        .await?;

    let _ = wait_for_event(
        &mut sender_events,
        Duration::from_secs(10),
        |event| match event {
            nooboard_app::AppEvent::TransferCompleted {
                transfer_id,
                outcome: nooboard_app::TransferOutcome::Succeeded,
            } if transfer_id == sender_transfer_id => Some(transfer_id),
            _ => None,
        },
    )
    .await?;
    let _ = wait_for_event(
        &mut receiver_events,
        Duration::from_secs(10),
        |event| match event {
            nooboard_app::AppEvent::TransferCompleted {
                transfer_id,
                outcome: nooboard_app::TransferOutcome::Succeeded,
            } if transfer_id == receiver_transfer_id => Some(transfer_id),
            _ => None,
        },
    )
    .await?;

    let receiver_state = wait_for_service_state(&restarted_b, Duration::from_secs(10), |state| {
        state.transfers.recent_completed.iter().any(|completed| {
            completed.transfer_id == receiver_transfer_id && completed.saved_path.is_some()
        })
    })
    .await?;
    let completed = receiver_state
        .transfers
        .recent_completed
        .iter()
        .find(|completed| completed.transfer_id == receiver_transfer_id)
        .expect("receiver must expose completed transfer");
    let saved_path = completed
        .saved_path
        .as_ref()
        .expect("download must have saved path");
    assert!(saved_path.starts_with(&custom_download_dir));
    assert_eq!(std::fs::read_to_string(saved_path)?, "settings-download");

    service_a.shutdown().await?;
    restarted_b.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_settings_publish_to_state_subscription_and_persist_across_restart()
-> Result<(), TestError> {
    let env = new_service()?;
    let service = &env.service;
    let mut state_subscription = service.subscribe_state().await?;
    let initial_revision = state_subscription.latest().revision;
    let manual_peers: Vec<SocketAddr> =
        vec!["127.0.0.1:24001".parse()?, "127.0.0.1:24002".parse()?];

    service
        .patch_settings(SettingsPatch::Network(
            NetworkSettingsPatch::SetNetworkEnabled(false),
        ))
        .await?;
    let state_after_network = timeout(Duration::from_secs(2), state_subscription.recv()).await??;
    assert_eq!(state_after_network.revision, initial_revision + 1);
    assert!(!state_after_network.settings.network.network_enabled);
    assert!(state_after_network.settings.network.mdns_enabled);
    assert!(state_after_network.settings.network.manual_peers.is_empty());

    service
        .patch_settings(SettingsPatch::Network(
            NetworkSettingsPatch::SetMdnsEnabled(false),
        ))
        .await?;
    let state_after_mdns = timeout(Duration::from_secs(2), state_subscription.recv()).await??;
    assert_eq!(state_after_mdns.revision, initial_revision + 2);
    assert!(!state_after_mdns.settings.network.network_enabled);
    assert!(!state_after_mdns.settings.network.mdns_enabled);
    assert!(state_after_mdns.settings.network.manual_peers.is_empty());

    service
        .patch_settings(SettingsPatch::Network(
            NetworkSettingsPatch::SetManualPeers(manual_peers.clone()),
        ))
        .await?;
    let state_after_manual_peers =
        timeout(Duration::from_secs(2), state_subscription.recv()).await??;
    assert_eq!(state_after_manual_peers.revision, initial_revision + 3);
    assert!(!state_after_manual_peers.settings.network.network_enabled);
    assert!(!state_after_manual_peers.settings.network.mdns_enabled);
    assert_eq!(
        state_after_manual_peers.settings.network.manual_peers,
        manual_peers
    );
    assert_eq!(
        service.get_state().await?.settings.network,
        state_after_manual_peers.settings.network
    );

    service.shutdown().await?;

    let restarted_backend = Arc::new(MockClipboardBackend::default());
    let restarted = restart_service(&env.config_path, restarted_backend)?;
    let restarted_network = restarted.get_state().await?.settings.network;
    assert!(!restarted_network.network_enabled);
    assert!(!restarted_network.mdns_enabled);
    assert_eq!(restarted_network.manual_peers, manual_peers);

    restarted.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn storage_settings_persist_across_restart_and_reconfigure_history_backend()
-> Result<(), TestError> {
    let env = new_service()?;
    let service = &env.service;
    let switched_db_root = PathBuf::from("db-alt");
    let expected_db_root = env.dir.path().join(&switched_db_root);

    let original_event = service
        .submit_text(SubmitTextRequest {
            content: "before-db-switch".to_string(),
        })
        .await?;

    service
        .patch_settings(SettingsPatch::Storage(StorageSettingsPatch {
            db_root: Some(switched_db_root),
            max_text_bytes: Some(4),
            ..StorageSettingsPatch::default()
        }))
        .await?;

    let storage_settings = service.get_state().await?.settings.storage;
    assert_eq!(storage_settings.db_root, expected_db_root);
    assert_eq!(storage_settings.max_text_bytes, 4);

    match service.get_clipboard_record(original_event).await {
        Err(AppError::EventNotFound { event_id }) => {
            assert_eq!(event_id, original_event.to_string());
        }
        other => return Err(format!("expected EventNotFound after db switch, got {other:?}").into()),
    }

    let switched_event = service
        .submit_text(SubmitTextRequest {
            content: "okok".to_string(),
        })
        .await?;
    let history_after_switch = service
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(history_after_switch.records.len(), 1);
    assert_eq!(history_after_switch.records[0].event_id, switched_event);
    assert_eq!(history_after_switch.records[0].content, "okok");

    service.shutdown().await?;

    let restarted_backend = Arc::new(MockClipboardBackend::default());
    let restarted = restart_service(&env.config_path, restarted_backend)?;
    let restarted_storage = restarted.get_state().await?.settings.storage;
    assert_eq!(restarted_storage.db_root, expected_db_root);
    assert_eq!(restarted_storage.max_text_bytes, 4);

    match restarted
        .submit_text(SubmitTextRequest {
            content: "12345".to_string(),
        })
        .await
    {
        Err(AppError::TextTooLarge {
            actual_bytes,
            max_bytes,
        }) => {
            assert_eq!(actual_bytes, 5);
            assert_eq!(max_bytes, 4);
        }
        other => return Err(format!("expected TextTooLarge after restart, got {other:?}").into()),
    }

    match restarted.get_clipboard_record(original_event).await {
        Err(AppError::EventNotFound { event_id }) => {
            assert_eq!(event_id, original_event.to_string());
        }
        other => {
            return Err(
                format!("expected old db event to stay hidden after restart, got {other:?}")
                    .into(),
            );
        }
    }

    let persisted_record = restarted.get_clipboard_record(switched_event).await?;
    assert_eq!(persisted_record.content, "okok");

    restarted.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_settings_patch_does_not_mutate_state_or_persist() -> Result<(), TestError> {
    let env = new_service()?;
    let service = &env.service;
    let baseline_settings = service.get_state().await?.settings;
    let mut state_subscription = service.subscribe_state().await?;
    let duplicate_peer: SocketAddr = "127.0.0.1:24077".parse()?;

    let error = service
        .patch_settings(SettingsPatch::Network(
            NetworkSettingsPatch::SetManualPeers(vec![duplicate_peer, duplicate_peer]),
        ))
        .await
        .expect_err("duplicate manual peers must fail validation");
    assert!(matches!(error, AppError::InvalidConfig(ref message) if message.contains("duplicate address")));

    let maybe_state = timeout(Duration::from_millis(200), state_subscription.recv()).await;
    assert!(
        maybe_state.is_err(),
        "failed validation must not publish a new app state"
    );
    assert_eq!(service.get_state().await?.settings, baseline_settings);

    service.shutdown().await?;

    let restarted_backend = Arc::new(MockClipboardBackend::default());
    let restarted = restart_service(&env.config_path, restarted_backend)?;
    assert_eq!(restarted.get_state().await?.settings, baseline_settings);

    restarted.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_disabled_reports_disabled_actual_and_rejects_sync_actions()
-> Result<(), TestError> {
    let env = new_service()?;
    let service = &env.service;

    service
        .patch_settings(SettingsPatch::Network(
            NetworkSettingsPatch::SetNetworkEnabled(false),
        ))
        .await?;
    service
        .set_sync_desired_state(SyncDesiredState::Running)
        .await?;

    let disabled_state = wait_for_service_state(service, Duration::from_secs(10), |state| {
        state.sync.desired == SyncDesiredState::Running
            && state.sync.actual == SyncActualStatus::Disabled
            && !state.settings.network.network_enabled
    })
    .await?;
    assert!(disabled_state.peers.connected.is_empty());

    let event_id = service
        .submit_text(SubmitTextRequest {
            content: "still-local".to_string(),
        })
        .await?;
    let source_file = env.dir.path().join("disabled-sync.txt");
    std::fs::write(&source_file, b"disabled-sync")?;

    let rebroadcast_error = service
        .rebroadcast_clipboard_record(nooboard_app::RebroadcastClipboardRequest {
            event_id,
            targets: nooboard_app::ClipboardBroadcastTargets::AllConnected,
        })
        .await
        .expect_err("rebroadcast must fail when network is disabled");
    assert!(matches!(rebroadcast_error, AppError::SyncDisabled));

    let send_error = service
        .send_files(SendFilesRequest {
            targets: vec![NoobId::new("offline-node")],
            files: vec![SendFileItem { path: source_file }],
        })
        .await
        .expect_err("send_files must fail when network is disabled");
    assert!(matches!(send_error, AppError::SyncDisabled));

    service
        .patch_settings(SettingsPatch::Network(
            NetworkSettingsPatch::SetNetworkEnabled(true),
        ))
        .await?;
    let reenabled_state = wait_for_service_state(service, Duration::from_secs(10), |state| {
        state.sync.desired == SyncDesiredState::Running
            && state.sync.actual == SyncActualStatus::Running
            && state.settings.network.network_enabled
    })
    .await?;
    assert!(reenabled_state.settings.network.network_enabled);

    service.shutdown().await?;
    Ok(())
}
