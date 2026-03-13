use std::fs;
use std::path::PathBuf;

use tempfile::tempdir;
use tokio::time::{Duration, sleep, timeout};

use crate::config::{
    DirectConfig, LanConfig, LocalIdentityConfig, NetworkAuthConfig, NetworkTransferConfig,
    NetworkTransportConfig,
};
use crate::{
    ConnectDirectOutcome, IncomingTransferDecision, IncomingTransferDisposition, NetworkConfig,
    NetworkError, NetworkEvent, NetworkRuntime, NetworkStatus, PendingDirectRequest,
    SendTextRequest, SessionId, SessionInfo, SessionTarget, UpsertDirectSeedInput,
};

fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .expect("bind free port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn test_config_named(
    noob_id: &str,
    device_id: &str,
    download_dir: PathBuf,
    lan_enabled: bool,
) -> NetworkConfig {
    NetworkConfig {
        identity: LocalIdentityConfig {
            noob_id: noob_id.to_string(),
            device_id: device_id.to_string(),
        },
        listen_port: free_port(),
        auth: NetworkAuthConfig {
            token: "token".to_string(),
        },
        lan: LanConfig {
            enabled: lan_enabled,
        },
        direct: DirectConfig {
            approval_timeout_ms: 30_000,
            seeds: vec![],
        },
        transport: NetworkTransportConfig {
            connect_timeout_ms: 5_000,
            handshake_timeout_ms: 5_000,
            ping_interval_ms: 5_000,
            pong_timeout_ms: 15_000,
            max_packet_size: 8 * 1024 * 1024,
        },
        transfer: NetworkTransferConfig {
            download_dir,
            max_file_size: 1024 * 1024,
            chunk_size: 512,
            active_downloads: 4,
            decision_timeout_ms: 30_000,
            idle_timeout_ms: 15_000,
        },
    }
}

fn test_config() -> NetworkConfig {
    let temp = tempdir().expect("tempdir");
    test_config_named("node-a", "device-a", temp.path().to_path_buf(), true)
}

async fn wait_for_session_count(runtime: &NetworkRuntime, expected: usize) -> Vec<SessionInfo> {
    timeout(Duration::from_secs(5), async {
        loop {
            let sessions = runtime.list_sessions().await.expect("sessions");
            if sessions.len() == expected {
                return sessions;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("wait for sessions")
}

async fn wait_for_pending_request(runtime: &NetworkRuntime) -> PendingDirectRequest {
    timeout(Duration::from_secs(5), async {
        loop {
            let pending = runtime
                .list_pending_direct_requests()
                .await
                .expect("pending requests");
            if let Some(request) = pending.into_iter().next() {
                return request;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("wait for pending request")
}

async fn wait_for_event<F>(
    subscription: &mut crate::NetworkSubscription,
    mut predicate: F,
) -> NetworkEvent
where
    F: FnMut(&NetworkEvent) -> bool,
{
    timeout(Duration::from_secs(10), async {
        loop {
            let event = subscription.recv().await.expect("event");
            if predicate(&event) {
                return event;
            }
        }
    })
    .await
    .expect("wait for event")
}

#[tokio::test]
async fn start_and_shutdown_are_idempotent() {
    let runtime = NetworkRuntime::new(test_config()).expect("runtime");
    runtime.start().await.expect("start");
    runtime.start().await.expect("start again");
    assert!(matches!(
        runtime.snapshot().await.expect("snapshot").status,
        NetworkStatus::Running
    ));

    runtime.shutdown().await.expect("stop");
    runtime.shutdown().await.expect("stop again");
    assert!(matches!(
        runtime.snapshot().await.expect("snapshot").status,
        NetworkStatus::Stopped
    ));
}

#[tokio::test]
async fn set_lan_enabled_updates_stopped_runtime_without_error() {
    let runtime = NetworkRuntime::new(test_config()).expect("runtime");

    runtime
        .set_lan_enabled(false)
        .await
        .expect("disable lan while stopped");

    let snapshot = runtime.snapshot().await.expect("snapshot");
    assert!(!snapshot.lan_enabled);
    assert!(matches!(snapshot.status, NetworkStatus::Stopped));
}

#[tokio::test]
async fn send_text_uses_public_runtime_validation() {
    let runtime = NetworkRuntime::new(test_config()).expect("runtime");
    runtime.start().await.expect("start");

    let error = runtime
        .send_text(SendTextRequest {
            event_id: "evt".to_string(),
            content: "hello".to_string(),
            target: SessionTarget::Sessions(vec![SessionId::new()]),
        })
        .await
        .expect_err("missing session should fail");

    assert!(matches!(error, NetworkError::SessionNotFound(_)));
}

#[tokio::test]
async fn direct_connect_establishes_session_after_approval() {
    let temp_a = tempdir().expect("tempdir");
    let temp_b = tempdir().expect("tempdir");
    let runtime_a = NetworkRuntime::new(test_config_named(
        "node-a",
        "device-a",
        temp_a.path().to_path_buf(),
        false,
    ))
    .expect("runtime a");
    let config_b = test_config_named("node-b", "device-b", temp_b.path().to_path_buf(), false);
    let port_b = config_b.listen_port;
    let runtime_b = NetworkRuntime::new(config_b).expect("runtime b");

    runtime_a.start().await.expect("start a");
    runtime_b.start().await.expect("start b");

    let seed_id = runtime_a
        .upsert_direct_seed(UpsertDirectSeedInput {
            id: None,
            label: "peer-b".to_string(),
            host: "127.0.0.1".to_string(),
            port: port_b,
            enabled: true,
        })
        .await
        .expect("seed");

    let outcome = runtime_a
        .connect_direct_seed(seed_id)
        .await
        .expect("connect");
    assert!(matches!(outcome, ConnectDirectOutcome::Started));

    let pending = wait_for_pending_request(&runtime_b).await;
    runtime_b
        .approve_direct_request(pending.id)
        .await
        .expect("approve");

    let sessions_a = wait_for_session_count(&runtime_a, 1).await;
    let sessions_b = wait_for_session_count(&runtime_b, 1).await;

    assert_eq!(sessions_a[0].mode, crate::ConnectionMode::Direct);
    assert_eq!(sessions_b[0].mode, crate::ConnectionMode::Direct);

    runtime_a.shutdown().await.expect("shutdown a");
    runtime_b.shutdown().await.expect("shutdown b");
}

#[tokio::test]
async fn direct_connect_returns_already_connected_for_existing_session() {
    let temp_a = tempdir().expect("tempdir");
    let temp_b = tempdir().expect("tempdir");
    let runtime_a = NetworkRuntime::new(test_config_named(
        "node-a",
        "device-a",
        temp_a.path().to_path_buf(),
        false,
    ))
    .expect("runtime a");
    let config_b = test_config_named("node-b", "device-b", temp_b.path().to_path_buf(), false);
    let port_b = config_b.listen_port;
    let runtime_b = NetworkRuntime::new(config_b).expect("runtime b");

    runtime_a.start().await.expect("start a");
    runtime_b.start().await.expect("start b");

    let seed_id = runtime_a
        .upsert_direct_seed(UpsertDirectSeedInput {
            id: None,
            label: "peer-b".to_string(),
            host: "127.0.0.1".to_string(),
            port: port_b,
            enabled: true,
        })
        .await
        .expect("seed");

    runtime_a
        .connect_direct_seed(seed_id)
        .await
        .expect("connect");
    let pending = wait_for_pending_request(&runtime_b).await;
    runtime_b
        .approve_direct_request(pending.id)
        .await
        .expect("approve");

    let sessions_a = wait_for_session_count(&runtime_a, 1).await;

    let outcome = runtime_a
        .connect_direct_seed(seed_id)
        .await
        .expect("connect again");
    assert!(matches!(
        outcome,
        ConnectDirectOutcome::AlreadyConnected(id) if id == sessions_a[0].id
    ));

    runtime_a.shutdown().await.expect("shutdown a");
    runtime_b.shutdown().await.expect("shutdown b");
}

#[tokio::test]
async fn removing_direct_seed_does_not_disconnect_active_session() {
    let temp_a = tempdir().expect("tempdir");
    let temp_b = tempdir().expect("tempdir");
    let runtime_a = NetworkRuntime::new(test_config_named(
        "node-a",
        "device-a",
        temp_a.path().to_path_buf(),
        false,
    ))
    .expect("runtime a");
    let config_b = test_config_named("node-b", "device-b", temp_b.path().to_path_buf(), false);
    let port_b = config_b.listen_port;
    let runtime_b = NetworkRuntime::new(config_b).expect("runtime b");

    runtime_a.start().await.expect("start a");
    runtime_b.start().await.expect("start b");

    let seed_id = runtime_a
        .upsert_direct_seed(UpsertDirectSeedInput {
            id: None,
            label: "peer-b".to_string(),
            host: "127.0.0.1".to_string(),
            port: port_b,
            enabled: true,
        })
        .await
        .expect("seed");

    runtime_a
        .connect_direct_seed(seed_id)
        .await
        .expect("connect");
    let pending = wait_for_pending_request(&runtime_b).await;
    runtime_b
        .approve_direct_request(pending.id)
        .await
        .expect("approve");

    wait_for_session_count(&runtime_a, 1).await;
    wait_for_session_count(&runtime_b, 1).await;

    runtime_a
        .remove_direct_seed(seed_id)
        .await
        .expect("remove seed");

    assert!(
        runtime_a
            .list_direct_seeds()
            .await
            .expect("list seeds")
            .is_empty()
    );
    assert_eq!(
        runtime_a.list_sessions().await.expect("sessions a").len(),
        1
    );
    assert!(matches!(
        runtime_a.connect_direct_seed(seed_id).await.expect_err("seed removed"),
        NetworkError::DirectSeedNotFound(id) if id == seed_id
    ));

    runtime_a.shutdown().await.expect("shutdown a");
    runtime_b.shutdown().await.expect("shutdown b");
}

#[tokio::test]
async fn direct_rejection_reports_failure_without_session() {
    let temp_a = tempdir().expect("tempdir");
    let temp_b = tempdir().expect("tempdir");
    let runtime_a = NetworkRuntime::new(test_config_named(
        "node-a",
        "device-a",
        temp_a.path().to_path_buf(),
        false,
    ))
    .expect("runtime a");
    let config_b = test_config_named("node-b", "device-b", temp_b.path().to_path_buf(), false);
    let port_b = config_b.listen_port;
    let runtime_b = NetworkRuntime::new(config_b).expect("runtime b");

    runtime_a.start().await.expect("start a");
    runtime_b.start().await.expect("start b");

    let seed_id = runtime_a
        .upsert_direct_seed(UpsertDirectSeedInput {
            id: None,
            label: "peer-b".to_string(),
            host: "127.0.0.1".to_string(),
            port: port_b,
            enabled: true,
        })
        .await
        .expect("seed");

    let mut sub_a = runtime_a.subscribe();

    runtime_a
        .connect_direct_seed(seed_id)
        .await
        .expect("connect");
    let pending = wait_for_pending_request(&runtime_b).await;
    runtime_b
        .reject_direct_request(pending.id)
        .await
        .expect("reject");

    let failure = wait_for_event(&mut sub_a, |event| {
        matches!(
            event,
            NetworkEvent::ConnectionFailed(failure)
                if failure.kind == crate::ConnectionFailureKind::DirectRejected
        )
    })
    .await;
    let NetworkEvent::ConnectionFailed(failure) = failure else {
        unreachable!();
    };
    assert_eq!(failure.kind, crate::ConnectionFailureKind::DirectRejected);
    assert!(runtime_a.list_sessions().await.expect("sessions a").is_empty());
    assert!(runtime_b.list_sessions().await.expect("sessions b").is_empty());
    assert!(
        runtime_b
            .list_pending_direct_requests()
            .await
            .expect("pending")
            .is_empty()
    );

    runtime_a.shutdown().await.expect("shutdown a");
    runtime_b.shutdown().await.expect("shutdown b");
}

#[tokio::test]
async fn direct_approval_timeout_expires_request_and_reports_failure() {
    let temp_a = tempdir().expect("tempdir");
    let temp_b = tempdir().expect("tempdir");
    let runtime_a = NetworkRuntime::new(test_config_named(
        "node-a",
        "device-a",
        temp_a.path().to_path_buf(),
        false,
    ))
    .expect("runtime a");
    let mut config_b = test_config_named("node-b", "device-b", temp_b.path().to_path_buf(), false);
    config_b.direct.approval_timeout_ms = 200;
    let port_b = config_b.listen_port;
    let runtime_b = NetworkRuntime::new(config_b).expect("runtime b");

    runtime_a.start().await.expect("start a");
    runtime_b.start().await.expect("start b");

    let seed_id = runtime_a
        .upsert_direct_seed(UpsertDirectSeedInput {
            id: None,
            label: "peer-b".to_string(),
            host: "127.0.0.1".to_string(),
            port: port_b,
            enabled: true,
        })
        .await
        .expect("seed");

    let mut sub_a = runtime_a.subscribe();

    runtime_a
        .connect_direct_seed(seed_id)
        .await
        .expect("connect");
    let _pending = wait_for_pending_request(&runtime_b).await;

    let failure = wait_for_event(&mut sub_a, |event| {
        matches!(
            event,
            NetworkEvent::ConnectionFailed(failure)
                if failure.kind == crate::ConnectionFailureKind::DirectExpired
        )
    })
    .await;
    let NetworkEvent::ConnectionFailed(failure) = failure else {
        unreachable!();
    };
    assert_eq!(failure.kind, crate::ConnectionFailureKind::DirectExpired);

    timeout(Duration::from_secs(5), async {
        loop {
            if runtime_b
                .list_pending_direct_requests()
                .await
                .expect("pending")
                .is_empty()
            {
                break;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("pending requests cleared");

    runtime_a.shutdown().await.expect("shutdown a");
    runtime_b.shutdown().await.expect("shutdown b");
}

#[tokio::test]
async fn direct_text_and_file_transfer_roundtrip() {
    let temp_a = tempdir().expect("tempdir");
    let temp_b = tempdir().expect("tempdir");
    let runtime_a = NetworkRuntime::new(test_config_named(
        "node-a",
        "device-a",
        temp_a.path().to_path_buf(),
        false,
    ))
    .expect("runtime a");
    let config_b = test_config_named("node-b", "device-b", temp_b.path().to_path_buf(), false);
    let port_b = config_b.listen_port;
    let runtime_b = NetworkRuntime::new(config_b).expect("runtime b");

    runtime_a.start().await.expect("start a");
    runtime_b.start().await.expect("start b");

    let seed_id = runtime_a
        .upsert_direct_seed(UpsertDirectSeedInput {
            id: None,
            label: "peer-b".to_string(),
            host: "127.0.0.1".to_string(),
            port: port_b,
            enabled: true,
        })
        .await
        .expect("seed");

    let mut sub_a = runtime_a.subscribe();
    let mut sub_b = runtime_b.subscribe();

    runtime_a
        .connect_direct_seed(seed_id)
        .await
        .expect("connect");
    let pending = wait_for_pending_request(&runtime_b).await;
    runtime_b
        .approve_direct_request(pending.id)
        .await
        .expect("approve");

    wait_for_session_count(&runtime_a, 1).await;
    wait_for_session_count(&runtime_b, 1).await;

    runtime_a
        .send_text(SendTextRequest {
            event_id: "evt-1".to_string(),
            content: "hello direct".to_string(),
            target: SessionTarget::AllConnected,
        })
        .await
        .expect("send text");

    let text_event = wait_for_event(&mut sub_b, |event| {
        matches!(event, NetworkEvent::TextReceived { event_id, .. } if event_id == "evt-1")
    })
    .await;
    let NetworkEvent::TextReceived { content, .. } = text_event else {
        unreachable!();
    };
    assert_eq!(content, "hello direct");

    let source_path = temp_a.path().join("demo.txt");
    fs::write(&source_path, b"file-body").expect("write source");

    let tickets = runtime_a
        .send_files(crate::SendFilesRequest {
            files: vec![source_path.clone()],
            target: SessionTarget::AllConnected,
        })
        .await
        .expect("send files");
    assert_eq!(tickets.len(), 1);

    let offer_event =
        wait_for_event(&mut sub_b, |event| matches!(event, NetworkEvent::IncomingTransferOffered { .. }))
            .await;
    let NetworkEvent::IncomingTransferOffered { offer } = offer_event else {
        unreachable!();
    };
    runtime_b
        .decide_incoming_transfer(IncomingTransferDecision {
            ticket: offer.ticket,
            decision: IncomingTransferDisposition::Accept,
        })
        .await
        .expect("accept transfer");

    let completed_b = wait_for_event(&mut sub_b, |event| {
        matches!(
            event,
            NetworkEvent::TransferCompleted { transfer } if transfer.ticket == offer.ticket
        )
    })
    .await;
    let NetworkEvent::TransferCompleted { transfer } = completed_b else {
        unreachable!();
    };
    let saved_path = transfer.saved_path.expect("saved path");
    assert_eq!(fs::read(saved_path).expect("read file"), b"file-body");

    let _completed_a = wait_for_event(&mut sub_a, |event| {
        matches!(
            event,
            NetworkEvent::TransferCompleted { transfer } if transfer.ticket == tickets[0]
        )
    })
    .await;

    runtime_a.shutdown().await.expect("shutdown a");
    runtime_b.shutdown().await.expect("shutdown b");
}
