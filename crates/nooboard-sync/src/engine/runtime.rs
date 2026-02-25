use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::{MissedTickBehavior, interval};
use tokio_rustls::TlsStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{debug, error, info, warn};

use crate::auth::ChallengeRegistry;
use crate::config::SyncConfig;
use crate::session::actor::{SessionActorContext, SessionCommand, run_session_actor};
use crate::discovery::{
    DiscoveredPeer, MdnsDiscoveryConfig, MdnsHandle, start_mdns_discovery,
};
use crate::error::SyncError;
use crate::transport::TlsContext;

use super::connect::{connection_direction_allowed, schedule_connect_attempts};
use super::ingress::{run_accept_loop, run_discovery_forward_loop};
use super::policy::{DedupeDecision, dedupe_decision};
use super::peers::{EngineControl, PeerHandle, PeerRegistry};
use super::types::{
    FileDecisionInput, SyncControlCommand, SyncEngineHandle, SyncEvent, SyncStatus, TransferUpdate,
};

pub async fn start_sync_engine(config: SyncConfig) -> Result<SyncEngineHandle, SyncError> {
    start_sync_engine_with_discovery(config, None).await
}

pub async fn start_sync_engine_with_discovery(
    config: SyncConfig,
    discovery_rx: Option<mpsc::Receiver<DiscoveredPeer>>,
) -> Result<SyncEngineHandle, SyncError> {
    if let Err(message) = config.validate() {
        return Err(SyncError::InvalidConfig(message));
    }

    std::fs::create_dir_all(&config.download_dir)?;

    let (text_tx, text_rx) = mpsc::channel(128);
    let (file_tx, file_rx) = mpsc::channel(32);
    let (decision_tx, decision_rx) = mpsc::channel(128);
    let (control_tx, control_rx) = mpsc::channel(64);
    let (event_tx, event_rx) = mpsc::channel(128);
    let (progress_tx, progress_rx) = broadcast::channel::<TransferUpdate>(256);
    let (status_tx, status_rx) = watch::channel(if config.enabled {
        SyncStatus::Starting
    } else {
        SyncStatus::Disabled
    });
    let (shutdown_tx, _) = broadcast::channel(8);

    if config.enabled {
        tokio::spawn(run_engine(
            config,
            text_rx,
            file_rx,
            decision_rx,
            control_rx,
            event_tx,
            progress_tx,
            status_tx,
            shutdown_tx.clone(),
            discovery_rx,
        ));
    }

    Ok(SyncEngineHandle {
        text_tx,
        file_tx,
        decision_tx,
        control_tx,
        event_rx,
        progress_rx,
        status_rx,
        shutdown_tx,
    })
}

async fn run_engine(
    config: SyncConfig,
    mut text_rx: mpsc::Receiver<String>,
    mut file_rx: mpsc::Receiver<PathBuf>,
    mut decision_rx: mpsc::Receiver<FileDecisionInput>,
    mut control_rx: mpsc::Receiver<SyncControlCommand>,
    event_tx: mpsc::Sender<SyncEvent>,
    progress_tx: broadcast::Sender<TransferUpdate>,
    status_tx: watch::Sender<SyncStatus>,
    shutdown_tx: broadcast::Sender<()>,
    mut discovery_rx: Option<mpsc::Receiver<DiscoveredPeer>>,
) {
    let result = run_engine_inner(
        config,
        &mut text_rx,
        &mut file_rx,
        &mut decision_rx,
        &mut control_rx,
        event_tx,
        progress_tx,
        &status_tx,
        shutdown_tx,
        &mut discovery_rx,
    )
    .await;

    if let Err(error) = result {
        let _ = status_tx.send(SyncStatus::Error(error.to_string()));
        error!("sync engine stopped with error: {error}");
    } else {
        let _ = status_tx.send(SyncStatus::Stopped);
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_engine_inner(
    config: SyncConfig,
    text_rx: &mut mpsc::Receiver<String>,
    file_rx: &mut mpsc::Receiver<PathBuf>,
    decision_rx: &mut mpsc::Receiver<FileDecisionInput>,
    control_rx: &mut mpsc::Receiver<SyncControlCommand>,
    event_tx: mpsc::Sender<SyncEvent>,
    progress_tx: broadcast::Sender<TransferUpdate>,
    status_tx: &watch::Sender<SyncStatus>,
    shutdown_tx: broadcast::Sender<()>,
    discovery_rx: &mut Option<mpsc::Receiver<DiscoveredPeer>>,
) -> Result<(), SyncError> {
    let listener = TcpListener::bind(config.listen_addr).await?;
    let local_addr = listener.local_addr()?;

    info!(
        node_id = %config.noob_id,
        listen_addr = %local_addr,
        "sync engine listening"
    );

    let tls = TlsContext::ephemeral()?;
    let challenge_registry = Arc::new(ChallengeRegistry::new());
    let socket_counter = Arc::new(AtomicU64::new(1));

    let (engine_control_tx, mut engine_control_rx) = mpsc::channel::<EngineControl>(256);

    let accept_task = tokio::spawn(run_accept_loop(
        listener,
        config.clone(),
        tls.clone(),
        challenge_registry.clone(),
        engine_control_tx.clone(),
        shutdown_tx.subscribe(),
        socket_counter,
    ));

    let mut mdns_handle: Option<MdnsHandle> = None;
    if config.mdns_enabled {
        let (mdns_discovery_tx, mdns_discovery_rx) = mpsc::channel(128);
        let handle = start_mdns_discovery(
            MdnsDiscoveryConfig::new(config.noob_id.clone(), local_addr),
            mdns_discovery_tx,
            shutdown_tx.subscribe(),
        )?;
        mdns_handle = Some(handle);

        tokio::spawn(run_discovery_forward_loop(
            mdns_discovery_rx,
            engine_control_tx.clone(),
            shutdown_tx.subscribe(),
        ));
    }

    if let Some(rx) = discovery_rx.take() {
        tokio::spawn(run_discovery_forward_loop(
            rx,
            engine_control_tx.clone(),
            shutdown_tx.subscribe(),
        ));
    }

    let mut reconnect_timer = interval(Duration::from_millis(2_000));
    reconnect_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut registry = PeerRegistry::new();

    let _ = status_tx.send(SyncStatus::Running);

    let mut shutdown_rx = shutdown_tx.subscribe();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                debug!("sync engine received shutdown");
                break;
            }
            _ = reconnect_timer.tick() => {
                schedule_connect_attempts(
                    &config,
                    &mut registry,
                    &tls,
                    &engine_control_tx,
                );

                challenge_registry.prune_expired().await;
            }
            maybe_text = text_rx.recv() => {
                match maybe_text {
                    Some(text) => registry.broadcast_text(text).await,
                    None => break,
                }
            }
            maybe_path = file_rx.recv() => {
                match maybe_path {
                    Some(path) => registry.broadcast_file(path).await,
                    None => break,
                }
            }
            maybe_decision = decision_rx.recv() => {
                match maybe_decision {
                    Some(decision) => {
                        let peer_node_id = decision.peer_node_id.clone();
                        let transfer_id = decision.transfer_id;
                        if let Err(error) = registry.forward_file_decision(decision).await {
                            warn!(
                                peer=%peer_node_id,
                                transfer_id,
                                "drop file decision: {error}"
                            );
                            emit_connection_error_event(
                                &event_tx,
                                Some(peer_node_id),
                                None,
                                SyncError::Connection(error),
                            );
                        }
                    }
                    None => break,
                }
            }
            maybe_control_command = control_rx.recv() => {
                match maybe_control_command {
                    Some(command) => handle_sync_control_command(command, &mut registry).await,
                    None => break,
                }
            }
            maybe_control = engine_control_rx.recv() => {
                let Some(control) = maybe_control else {
                    break;
                };

                handle_engine_control(
                    control,
                    &config,
                    &event_tx,
                    &progress_tx,
                    &shutdown_tx,
                    &engine_control_tx,
                    &mut registry,
                ).await;
            }
        }
    }

    registry.shutdown_all().await;

    let _ = shutdown_tx.send(());
    accept_task.abort();
    if let Some(handle) = mdns_handle {
        handle.shutdown().await;
    }

    Ok(())
}

async fn handle_sync_control_command(command: SyncControlCommand, registry: &mut PeerRegistry) {
    match command {
        SyncControlCommand::DisconnectPeer { peer_node_id } => {
            if let Some(addr) = registry.disconnect_peer(&peer_node_id).await {
                info!(peer=%peer_node_id, addr=%addr, "disconnect peer requested by control channel");
            } else {
                warn!(peer=%peer_node_id, "disconnect peer requested but peer is not connected");
            }
        }
    }
}

async fn handle_engine_control(
    control: EngineControl,
    config: &SyncConfig,
    event_tx: &mpsc::Sender<SyncEvent>,
    progress_tx: &broadcast::Sender<TransferUpdate>,
    shutdown_tx: &broadcast::Sender<()>,
    engine_control_tx: &mpsc::Sender<EngineControl>,
    registry: &mut PeerRegistry,
) {
    match control {
        EngineControl::Connected {
            peer_node_id,
            addr,
            outbound,
            framed,
        } => {
            registry.clear_connecting(&addr);

            if peer_node_id == config.noob_id {
                error!(peer=%peer_node_id, "node_id conflict: local and remote are identical");
                return;
            }

            let decision = dedupe_decision(&config.noob_id, &peer_node_id);
            if !connection_direction_allowed(outbound, decision) {
                debug!(peer=%peer_node_id, outbound, "drop connection due to node_id dedupe direction");
                return;
            }

            if let Some(existing_outbound) = registry.peer_outbound(&peer_node_id) {
                if existing_outbound == outbound {
                    debug!(peer=%peer_node_id, "drop duplicate connection in same direction");
                    return;
                }

                if let Some(command_tx) = registry.peer_command_tx(&peer_node_id) {
                    let _ = command_tx.send(SessionCommand::Shutdown).await;
                }
            }

            let command_tx = spawn_session_actor_for_peer(
                config,
                event_tx,
                progress_tx,
                shutdown_tx,
                engine_control_tx,
                &peer_node_id,
                framed,
            );

            registry.insert_peer(
                peer_node_id,
                PeerHandle {
                    command_tx,
                    addr,
                    outbound,
                },
            );
        }
        EngineControl::ConnectFailed { addr, error } => {
            warn!(addr=%addr, "connection attempt failed: {error}");
            emit_connection_error_event(event_tx, None, Some(addr), error);
        }
        EngineControl::ConnectAttemptFinished { addr } => {
            registry.clear_connecting(&addr);
        }
        EngineControl::PeerFailed {
            peer_node_id,
            error,
        } => {
            warn!(peer=%peer_node_id, "session actor failed: {error}");
            emit_connection_error_event(
                event_tx,
                Some(peer_node_id),
                None,
                SyncError::Connection(error),
            );
        }
        EngineControl::PeerDisconnected { peer_node_id } => {
            registry.remove_peer(&peer_node_id);
        }
        EngineControl::DiscoveredPeer(peer) => {
            if matches!(
                registry.apply_discovered_peer(&config.noob_id, &peer),
                DedupeDecision::RejectConflict
            ) {
                error!(peer=%peer.node_id, "discovery conflict: local node_id equals remote node_id");
            }
        }
    }
}

fn spawn_session_actor_for_peer(
    config: &SyncConfig,
    event_tx: &mpsc::Sender<SyncEvent>,
    progress_tx: &broadcast::Sender<TransferUpdate>,
    shutdown_tx: &broadcast::Sender<()>,
    engine_control_tx: &mpsc::Sender<EngineControl>,
    peer_node_id: &str,
    framed: Framed<TlsStream<TcpStream>, LengthDelimitedCodec>,
) -> mpsc::Sender<SessionCommand> {
    let (command_tx, command_rx) = mpsc::channel(128);
    let disconnect_tx = engine_control_tx.clone();
    let peer_node_id_for_task = peer_node_id.to_string();

    let actor_config = config.clone();
    let actor_event_tx = event_tx.clone();
    let actor_progress_tx = progress_tx.clone();
    let actor_shutdown_tx = shutdown_tx.clone();

    tokio::spawn(async move {
        let actor_result = run_session_actor(SessionActorContext {
            peer_node_id: peer_node_id_for_task.clone(),
            config: actor_config,
            framed,
            command_rx,
            event_tx: actor_event_tx,
            progress_tx: actor_progress_tx,
            shutdown_rx: actor_shutdown_tx.subscribe(),
        })
        .await;

        if let Err(error) = actor_result {
            let _ = disconnect_tx
                .send(EngineControl::PeerFailed {
                    peer_node_id: peer_node_id_for_task.clone(),
                    error,
                })
                .await;
        }

        let _ = disconnect_tx
            .send(EngineControl::PeerDisconnected {
                peer_node_id: peer_node_id_for_task,
            })
            .await;
    });

    command_tx
}

fn emit_connection_error_event(
    event_tx: &mpsc::Sender<SyncEvent>,
    peer_node_id: Option<String>,
    addr: Option<std::net::SocketAddr>,
    error: SyncError,
) {
    if let Err(send_error) = event_tx.try_send(SyncEvent::ConnectionError {
        peer_node_id,
        addr,
        error: error.to_string(),
    }) {
        warn!("drop connection error event: {send_error}");
    }
}
