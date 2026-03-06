use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::auth::{ChallengeRegistry, SocketId};
use crate::config::SyncConfig;
use crate::discovery::DiscoveredPeer;
use crate::transport::TlsContext;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};

use super::connect::accept_inbound_peer;
use super::peers::EngineControl;

pub(super) async fn run_accept_loop(
    listener: TcpListener,
    config: SyncConfig,
    tls: TlsContext,
    challenge_registry: Arc<ChallengeRegistry>,
    control_tx: mpsc::Sender<EngineControl>,
    mut shutdown_rx: broadcast::Receiver<()>,
    socket_counter: Arc<AtomicU64>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                break;
            }
            accepted = listener.accept() => {
                let Ok((stream, addr)) = accepted else {
                    continue;
                };

                let config = config.clone();
                let tls = tls.clone();
                let challenge_registry = challenge_registry.clone();
                let control_tx = control_tx.clone();
                let socket_id = socket_counter.fetch_add(1, Ordering::Relaxed) as SocketId;

                tokio::spawn(async move {
                    let result = accept_inbound_peer(
                        stream,
                        addr,
                        socket_id,
                        &config,
                        &tls,
                        challenge_registry,
                    )
                    .await;

                    match result {
                        Ok((peer_noob_id, peer_device_id, framed)) => {
                            let _ = control_tx
                                .send(EngineControl::Connected {
                                    peer_noob_id,
                                    peer_device_id,
                                    addr,
                                    outbound: false,
                                    framed,
                                })
                                .await;
                        }
                        Err(error) => {
                            let _ = control_tx
                                .send(EngineControl::ConnectFailed { addr, error })
                                .await;
                        }
                    }
                });
            }
        }
    }
}

pub(super) async fn run_discovery_forward_loop(
    mut rx: mpsc::Receiver<DiscoveredPeer>,
    control_tx: mpsc::Sender<EngineControl>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            maybe_peer = rx.recv() => {
                let Some(peer) = maybe_peer else {
                    break;
                };

                let _ = control_tx.send(EngineControl::DiscoveredPeer(peer)).await;
            }
        }
    }
}
