use std::time::Duration;

use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use nooboard_sync::{SyncEvent, SyncStatus, TransferUpdate};

pub(super) fn spawn_event_bridge(
    mut event_rx: mpsc::Receiver<SyncEvent>,
    event_sender: broadcast::Sender<SyncEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let _ = event_sender.send(event);
        }
    })
}

pub(super) fn spawn_transfer_bridge(
    mut progress_rx: broadcast::Receiver<TransferUpdate>,
    transfer_sender: broadcast::Sender<TransferUpdate>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match progress_rx.recv().await {
                Ok(update) => {
                    let _ = transfer_sender.send(update);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

pub(super) fn spawn_peer_bridge(
    mut peers_rx: watch::Receiver<Vec<nooboard_sync::ConnectedPeerInfo>>,
    peers_sender: watch::Sender<Vec<nooboard_sync::ConnectedPeerInfo>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let _ = peers_sender.send(peers_rx.borrow().clone());
        while peers_rx.changed().await.is_ok() {
            let _ = peers_sender.send(peers_rx.borrow().clone());
        }
    })
}

pub(super) fn spawn_status_bridge(
    mut status_rx: watch::Receiver<SyncStatus>,
    status_sender: watch::Sender<SyncStatus>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let _ = status_sender.send(status_rx.borrow().clone());
        while status_rx.changed().await.is_ok() {
            let _ = status_sender.send(status_rx.borrow().clone());
        }
    })
}

pub(super) async fn wait_for_engine_termination(
    status_rx: &mut watch::Receiver<SyncStatus>,
    max_wait: Duration,
) {
    let wait = async {
        loop {
            match status_rx.borrow().clone() {
                SyncStatus::Stopped | SyncStatus::Disabled | SyncStatus::Error(_) => break,
                SyncStatus::Starting | SyncStatus::Running => {}
            }

            if status_rx.changed().await.is_err() {
                break;
            }
        }
    };

    let _ = timeout(max_wait, wait).await;
}

pub(super) async fn abort_bridge_task(task: JoinHandle<()>) {
    task.abort();
    let _ = task.await;
}
