use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use nooboard_sync::{
    SendFileRequest, SendTextRequest, SyncConfig, SyncControlCommand, SyncEvent, TransferState,
    start_sync_engine,
};
use tempfile::TempDir;
use tokio::fs;
use tokio::time::timeout;

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("must bind ephemeral port");
    let port = listener
        .local_addr()
        .expect("must resolve local addr")
        .port();
    drop(listener);
    port
}

fn make_config(
    node_id: &str,
    listen_port: u16,
    peer_port: u16,
    download_dir: PathBuf,
    max_file_size: u64,
) -> SyncConfig {
    SyncConfig {
        enabled: true,
        mdns_enabled: false,
        listen_addr: format!("127.0.0.1:{listen_port}")
            .parse()
            .expect("listen addr must parse"),
        token: "stage3-test-token".to_string(),
        manual_peers: vec![
            format!("127.0.0.1:{peer_port}")
                .parse()
                .expect("peer addr must parse"),
        ],
        protocol_version: nooboard_sync::protocol::PROTOCOL_VERSION,
        connect_timeout_ms: 1_000,
        handshake_timeout_ms: 1_000,
        ping_interval_ms: 500,
        pong_timeout_ms: 5_000,
        max_packet_size: 256 * 1024,
        file_chunk_size: 32 * 1024,
        file_decision_timeout_ms: 500,
        transfer_idle_timeout_ms: 5_000,
        download_dir,
        max_file_size,
        active_downloads: 4,
        noob_id: node_id.to_string(),
    }
}

async fn wait_running(
    status_rx: &mut tokio::sync::watch::Receiver<nooboard_sync::SyncStatus>,
) -> Result<(), Box<dyn std::error::Error>> {
    for _ in 0..20 {
        if matches!(&*status_rx.borrow(), nooboard_sync::SyncStatus::Running) {
            return Ok(());
        }
        status_rx.changed().await?;
    }

    Err("sync engine did not become running".into())
}

async fn wait_peer_count(
    peers_rx: &mut tokio::sync::watch::Receiver<Vec<nooboard_sync::ConnectedPeerInfo>>,
    expected: usize,
    timeout_duration: Duration,
) -> Result<Vec<nooboard_sync::ConnectedPeerInfo>, Box<dyn std::error::Error>> {
    if peers_rx.borrow().len() == expected {
        return Ok(peers_rx.borrow().clone());
    }

    let deadline = Instant::now() + timeout_duration;
    while Instant::now() < deadline {
        let remain = deadline.duration_since(Instant::now());
        if timeout(remain, peers_rx.changed()).await.is_err() {
            break;
        }

        if peers_rx.borrow().len() == expected {
            return Ok(peers_rx.borrow().clone());
        }
    }

    Err(format!("peer count did not reach {expected}").into())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_transfer_accept_path_works() -> Result<(), Box<dyn std::error::Error>> {
    let dir_a = TempDir::new()?;
    let dir_b = TempDir::new()?;

    let port_a = free_port();
    let port_b = free_port();

    let mut handle_a = start_sync_engine(make_config(
        "node-a",
        port_a,
        port_b,
        dir_a.path().to_path_buf(),
        1024 * 1024,
    ))
    .await?;
    let mut handle_b = start_sync_engine(make_config(
        "node-b",
        port_b,
        port_a,
        dir_b.path().to_path_buf(),
        1024 * 1024,
    ))
    .await?;

    wait_running(&mut handle_a.status_rx).await?;
    wait_running(&mut handle_b.status_rx).await?;

    tokio::time::sleep(Duration::from_millis(400)).await;

    let source_file = dir_a.path().join("hello.txt");
    fs::write(&source_file, b"hello stage3").await?;

    handle_a
        .file_tx
        .send(SendFileRequest {
            path: source_file.clone(),
            targets: None,
        })
        .await?;

    let mut downloaded = None;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let remain = deadline.duration_since(Instant::now());
        let event = timeout(remain, handle_b.event_rx.recv()).await?;
        if let Some(event) = event {
            match event {
                SyncEvent::FileDecisionRequired {
                    peer_node_id,
                    transfer_id,
                    ..
                } => {
                    handle_b
                        .decision_tx
                        .send(nooboard_sync::FileDecisionInput {
                            peer_node_id,
                            transfer_id,
                            accept: true,
                            reason: None,
                        })
                        .await?;
                }
                SyncEvent::TransferUpdate(update) => {
                    if let TransferState::Finished { path: Some(path) } = update.state {
                        downloaded = Some(path);
                        break;
                    }
                }
                SyncEvent::ConnectionError { .. } => {}
                SyncEvent::TextReceived(_) => {}
            }
        } else {
            break;
        }
    }

    let downloaded = downloaded.expect("must receive file downloaded event");
    let content = fs::read_to_string(downloaded).await?;
    assert_eq!(content, "hello stage3");

    let _ = handle_a.shutdown_tx.send(());
    let _ = handle_b.shutdown_tx.send(());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_reject_cleans_tmp_file() -> Result<(), Box<dyn std::error::Error>> {
    let dir_a = TempDir::new()?;
    let dir_b = TempDir::new()?;

    let port_a = free_port();
    let port_b = free_port();

    let mut handle_a = start_sync_engine(make_config(
        "node-a",
        port_a,
        port_b,
        dir_a.path().to_path_buf(),
        1024 * 1024,
    ))
    .await?;
    let mut handle_b = start_sync_engine(make_config(
        "node-b",
        port_b,
        port_a,
        dir_b.path().to_path_buf(),
        1024 * 1024,
    ))
    .await?;

    wait_running(&mut handle_a.status_rx).await?;
    wait_running(&mut handle_b.status_rx).await?;

    tokio::time::sleep(Duration::from_millis(400)).await;

    let source_file = dir_a.path().join("reject.txt");
    fs::write(&source_file, b"this file is too big").await?;
    handle_a
        .file_tx
        .send(SendFileRequest {
            path: source_file.clone(),
            targets: None,
        })
        .await?;

    let mut saw_decision_request = false;
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let remain = deadline.duration_since(Instant::now());
        let event = timeout(remain, handle_b.event_rx.recv()).await?;
        if let Some(event) = event {
            if let SyncEvent::FileDecisionRequired {
                peer_node_id,
                transfer_id,
                ..
            } = event
            {
                saw_decision_request = true;
                handle_b
                    .decision_tx
                    .send(nooboard_sync::FileDecisionInput {
                        peer_node_id,
                        transfer_id,
                        accept: false,
                        reason: Some("rejected in test".to_string()),
                    })
                    .await?;
                break;
            }
        } else {
            break;
        }
    }

    assert!(
        saw_decision_request,
        "must receive file decision request event"
    );
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut entries = fs::read_dir(dir_b.path()).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();
        assert!(
            !name.ends_with(".tmp"),
            "tmp file should be cleaned after reject"
        );
    }

    let _ = handle_a.shutdown_tx.send(());
    let _ = handle_b.shutdown_tx.send(());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn control_channel_can_disconnect_specific_peer() -> Result<(), Box<dyn std::error::Error>> {
    let dir_a = TempDir::new()?;
    let dir_b = TempDir::new()?;

    let port_a = free_port();
    let port_b = free_port();

    let mut handle_a = start_sync_engine(make_config(
        "node-a",
        port_a,
        port_b,
        dir_a.path().to_path_buf(),
        1024 * 1024,
    ))
    .await?;
    let mut handle_b = start_sync_engine(make_config(
        "node-b",
        port_b,
        port_a,
        dir_b.path().to_path_buf(),
        1024 * 1024,
    ))
    .await?;

    wait_running(&mut handle_a.status_rx).await?;
    wait_running(&mut handle_b.status_rx).await?;

    tokio::time::sleep(Duration::from_millis(500)).await;

    handle_a
        .control_tx
        .send(SyncControlCommand::DisconnectPeer {
            peer_node_id: "node-b".to_string(),
        })
        .await?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    handle_a
        .text_tx
        .send(SendTextRequest {
            event_id: "evt-after-disconnect".to_string(),
            content: "after-disconnect".to_string(),
            targets: None,
        })
        .await?;

    let deadline = Instant::now() + Duration::from_millis(900);
    while Instant::now() < deadline {
        let remain = deadline.duration_since(Instant::now());
        let maybe_event = timeout(remain, handle_b.event_rx.recv()).await;
        match maybe_event {
            Err(_) => break,
            Ok(Some(SyncEvent::TextReceived(text))) => {
                assert_ne!(
                    text, "after-disconnect",
                    "peer should be disconnected and not receive text before reconnect loop runs"
                );
            }
            Ok(Some(_)) => {}
            Ok(None) => break,
        }
    }

    let _ = handle_a.shutdown_tx.send(());
    let _ = handle_b.shutdown_tx.send(());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peers_snapshot_updates_on_connect_and_disconnect() -> Result<(), Box<dyn std::error::Error>>
{
    let dir_a = TempDir::new()?;
    let dir_b = TempDir::new()?;

    let port_a = free_port();
    let port_b = free_port();

    let mut handle_a = start_sync_engine(make_config(
        "node-a",
        port_a,
        port_b,
        dir_a.path().to_path_buf(),
        1024 * 1024,
    ))
    .await?;
    let mut handle_b = start_sync_engine(make_config(
        "node-b",
        port_b,
        port_a,
        dir_b.path().to_path_buf(),
        1024 * 1024,
    ))
    .await?;

    wait_running(&mut handle_a.status_rx).await?;
    wait_running(&mut handle_b.status_rx).await?;

    let peers_a = wait_peer_count(&mut handle_a.peers_rx, 1, Duration::from_secs(3)).await?;
    assert_eq!(peers_a[0].peer_node_id, "node-b");
    assert_eq!(peers_a[0].addr.port(), port_b);
    assert!(peers_a[0].outbound);
    assert!(peers_a[0].connected_at_ms > 0);

    let peers_b = wait_peer_count(&mut handle_b.peers_rx, 1, Duration::from_secs(3)).await?;
    assert_eq!(peers_b[0].peer_node_id, "node-a");
    assert_eq!(peers_b[0].addr.ip().to_string(), "127.0.0.1");
    assert!(!peers_b[0].outbound);
    assert!(peers_b[0].connected_at_ms > 0);

    handle_a
        .control_tx
        .send(SyncControlCommand::DisconnectPeer {
            peer_node_id: "node-b".to_string(),
        })
        .await?;

    let _ = wait_peer_count(&mut handle_a.peers_rx, 0, Duration::from_secs(1)).await?;
    let _ = wait_peer_count(&mut handle_b.peers_rx, 0, Duration::from_secs(1)).await?;

    let _ = handle_a.shutdown_tx.send(());
    let _ = handle_b.shutdown_tx.send(());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_peer_file_decision_emits_connection_error_event()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let listen_port = free_port();
    let peer_port = free_port();
    let mut config = make_config(
        "solo-node",
        listen_port,
        peer_port,
        dir.path().to_path_buf(),
        1024 * 1024,
    );
    config.manual_peers.clear();

    let mut handle = start_sync_engine(config).await?;
    wait_running(&mut handle.status_rx).await?;

    handle
        .decision_tx
        .send(nooboard_sync::FileDecisionInput {
            peer_node_id: "ghost-peer".to_string(),
            transfer_id: 42,
            accept: true,
            reason: None,
        })
        .await?;

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saw_connection_error = false;
    while Instant::now() < deadline {
        let remain = deadline.duration_since(Instant::now());
        let event = timeout(remain, handle.event_rx.recv()).await?;
        if let Some(SyncEvent::ConnectionError {
            peer_node_id,
            addr: _,
            error,
        }) = event
        {
            if peer_node_id.as_deref() == Some("ghost-peer")
                && error.contains("connection error")
                && error.contains("not connected")
            {
                saw_connection_error = true;
                break;
            }
        }
    }

    assert!(
        saw_connection_error,
        "must emit connection error event for missing peer decision"
    );

    let _ = handle.shutdown_tx.send(());
    Ok(())
}
