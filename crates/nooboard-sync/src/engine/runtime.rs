use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::{MissedTickBehavior, interval};
use tokio_rustls::TlsStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{debug, error, info, warn};

use crate::auth::ChallengeRegistry;
use crate::config::SyncConfig;
use crate::discovery::{DiscoveredPeer, MdnsDiscoveryConfig, MdnsHandle, start_mdns_discovery};
use crate::error::SyncError;
use crate::session::actor::{SessionActorContext, SessionCommand, run_session_actor};
use crate::transport::TlsContext;

use super::connect::{connection_direction_allowed, schedule_connect_attempts};
use super::ingress::{run_accept_loop, run_discovery_forward_loop};
use super::peers::{EngineControl, PeerHandle, PeerRegistry};
use super::policy::{DedupeDecision, dedupe_decision};
use super::types::{
    ConnectedPeerInfo, FileDecisionInput, SendFileCommand, SendTextRequest, SyncControlCommand,
    SyncEngineHandle, SyncEvent, SyncStatus, TransferUpdate,
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

    let (text_tx, text_rx) = mpsc::channel::<SendTextRequest>(128);
    let (file_tx, file_rx) = mpsc::channel::<SendFileCommand>(32);
    let (decision_tx, decision_rx) = mpsc::channel(128);
    let (control_tx, control_rx) = mpsc::channel(64);
    let (event_tx, event_rx) = mpsc::channel(128);
    let (progress_tx, progress_rx) = broadcast::channel::<TransferUpdate>(256);
    let (peers_tx, peers_rx) = watch::channel(Vec::<ConnectedPeerInfo>::new());
    let (status_tx, status_rx) = watch::channel(if config.enabled {
        SyncStatus::Starting
    } else {
        SyncStatus::Disabled
    });
    let (shutdown_tx, _) = broadcast::channel(8);
    let engine_task = if config.enabled {
        Some(tokio::spawn(run_engine(
            config,
            text_rx,
            file_rx,
            decision_rx,
            control_rx,
            event_tx,
            progress_tx,
            peers_tx,
            status_tx,
            shutdown_tx.clone(),
            discovery_rx,
        )))
    } else {
        None
    };

    Ok(SyncEngineHandle {
        text_tx,
        file_tx,
        decision_tx,
        control_tx,
        event_rx,
        progress_rx,
        peers_rx,
        status_rx,
        shutdown_tx,
        engine_task,
    })
}

async fn run_engine(
    config: SyncConfig,
    mut text_rx: mpsc::Receiver<SendTextRequest>,
    mut file_rx: mpsc::Receiver<SendFileCommand>,
    mut decision_rx: mpsc::Receiver<FileDecisionInput>,
    mut control_rx: mpsc::Receiver<SyncControlCommand>,
    event_tx: mpsc::Sender<SyncEvent>,
    progress_tx: broadcast::Sender<TransferUpdate>,
    peers_tx: watch::Sender<Vec<ConnectedPeerInfo>>,
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
        &peers_tx,
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
    text_rx: &mut mpsc::Receiver<SendTextRequest>,
    file_rx: &mut mpsc::Receiver<SendFileCommand>,
    decision_rx: &mut mpsc::Receiver<FileDecisionInput>,
    control_rx: &mut mpsc::Receiver<SyncControlCommand>,
    event_tx: mpsc::Sender<SyncEvent>,
    progress_tx: broadcast::Sender<TransferUpdate>,
    peers_tx: &watch::Sender<Vec<ConnectedPeerInfo>>,
    status_tx: &watch::Sender<SyncStatus>,
    shutdown_tx: broadcast::Sender<()>,
    discovery_rx: &mut Option<mpsc::Receiver<DiscoveredPeer>>,
) -> Result<(), SyncError> {
    let listener = TcpListener::bind(config.listen_addr).await?;
    let local_addr = listener.local_addr()?;

    info!(
        noob_id = %config.noob_id,
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
    let mut session_id_seed = 1_u64;

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
                    Some(request) => registry.send_text(request),
                    None => break,
                }
            }
            maybe_path = file_rx.recv() => {
                match maybe_path {
                    Some(command) => {
                        let result = registry
                            .send_file(command.request)
                            .map_err(SyncError::Connection);
                        let _ = command.reply.send(result);
                    }
                    None => break,
                }
            }
            maybe_decision = decision_rx.recv() => {
                match maybe_decision {
                    Some(decision) => {
                        let peer_noob_id = decision.peer_noob_id.clone();
                        let transfer_id = decision.transfer_id;
                        if let Err(error) = registry.forward_file_decision(decision) {
                            warn!(
                                peer=%peer_noob_id,
                                transfer_id,
                                "drop file decision: {error}"
                            );
                            emit_connection_error_event(
                                &event_tx,
                                Some(peer_noob_id),
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
                    Some(command) => {
                        let changed = handle_sync_control_command(command, &mut registry).await;
                        if changed {
                            publish_peer_snapshot(&registry, peers_tx);
                        }
                    }
                    None => break,
                }
            }
            maybe_control = engine_control_rx.recv() => {
                let Some(control) = maybe_control else {
                    break;
                };

                let changed = handle_engine_control(
                    control,
                    &config,
                    &event_tx,
                    &progress_tx,
                    &shutdown_tx,
                    &engine_control_tx,
                    &mut registry,
                    &mut session_id_seed,
                ).await;
                if changed {
                    publish_peer_snapshot(&registry, peers_tx);
                }
            }
        }
    }

    registry.shutdown_all();
    registry.clear_peers();
    publish_peer_snapshot(&registry, peers_tx);

    let _ = shutdown_tx.send(());
    accept_task.abort();
    if let Some(handle) = mdns_handle {
        handle.shutdown().await;
    }

    Ok(())
}

async fn handle_sync_control_command(
    command: SyncControlCommand,
    registry: &mut PeerRegistry,
) -> bool {
    match command {
        SyncControlCommand::DisconnectPeer { peer_noob_id } => {
            if let Some(addr) = registry.disconnect_peer(&peer_noob_id) {
                info!(peer=%peer_noob_id, addr=%addr, "disconnect peer requested by control channel");
                true
            } else {
                warn!(peer=%peer_noob_id, "disconnect peer requested but peer is not connected");
                false
            }
        }
        SyncControlCommand::CancelTransfer { request, reply } => {
            let result = registry
                .cancel_transfer(request)
                .await
                .map_err(SyncError::Connection);
            let _ = reply.send(result);
            false
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
    session_id_seed: &mut u64,
) -> bool {
    match control {
        EngineControl::Connected {
            peer_noob_id,
            peer_device_id,
            addr,
            outbound,
            framed,
        } => {
            registry.clear_connecting(&addr);

            if peer_noob_id == config.noob_id {
                error!(peer=%peer_noob_id, "noob_id conflict: local and remote are identical");
                return false;
            }

            let decision = dedupe_decision(&config.noob_id, &peer_noob_id);
            if !connection_direction_allowed(outbound, decision) {
                debug!(peer=%peer_noob_id, outbound, "drop connection due to noob_id dedupe direction");
                return false;
            }

            if let Some(existing_outbound) = registry.peer_outbound(&peer_noob_id) {
                if existing_outbound == outbound {
                    debug!(peer=%peer_noob_id, "drop duplicate connection in same direction");
                    return false;
                }

                if let Some(command_tx) = registry.peer_command_tx(&peer_noob_id) {
                    let _ = command_tx.try_send(SessionCommand::Shutdown);
                }
            }

            let session_id = *session_id_seed;
            *session_id_seed = session_id_seed.wrapping_add(1);
            let command_tx = spawn_session_actor_for_peer(
                config,
                event_tx,
                progress_tx,
                shutdown_tx,
                engine_control_tx,
                &peer_noob_id,
                &peer_device_id,
                session_id,
                framed,
            );

            registry.insert_peer(
                peer_noob_id,
                PeerHandle {
                    command_tx,
                    addr,
                    outbound,
                    device_id: peer_device_id,
                    session_id,
                    connected_at_ms: now_millis_u64(),
                },
            );
            true
        }
        EngineControl::ConnectFailed {
            addr,
            error,
            outbound,
        } => {
            if outbound {
                registry.note_connect_failure(&addr);
            }
            warn!(addr=%addr, "connection attempt failed: {error}");
            emit_connection_error_event(event_tx, None, Some(addr), error);
            false
        }
        EngineControl::ConnectAttemptFinished { addr } => {
            registry.clear_connecting(&addr);
            false
        }
        EngineControl::PeerFailed {
            peer_noob_id,
            session_id,
            error,
        } => {
            if !registry.peer_matches_session(&peer_noob_id, session_id) {
                debug!(peer=%peer_noob_id, session_id, "ignore stale peer failure from closed session");
                return false;
            }
            warn!(peer=%peer_noob_id, "session actor failed: {error}");
            emit_connection_error_event(
                event_tx,
                Some(peer_noob_id),
                None,
                SyncError::Connection(error),
            );
            false
        }
        EngineControl::PeerDisconnected {
            peer_noob_id,
            session_id,
        } => {
            if registry.remove_peer_if_session(&peer_noob_id, session_id) {
                true
            } else {
                debug!(peer=%peer_noob_id, session_id, "ignore stale peer disconnected from old session");
                false
            }
        }
        EngineControl::DiscoveredPeer(peer) => {
            if matches!(
                registry.apply_discovered_peer(&config.noob_id, &peer),
                DedupeDecision::RejectConflict
            ) {
                error!(peer=%peer.noob_id, "discovery conflict: local noob_id equals remote noob_id");
            }
            false
        }
    }
}

fn spawn_session_actor_for_peer(
    config: &SyncConfig,
    event_tx: &mpsc::Sender<SyncEvent>,
    progress_tx: &broadcast::Sender<TransferUpdate>,
    shutdown_tx: &broadcast::Sender<()>,
    engine_control_tx: &mpsc::Sender<EngineControl>,
    peer_noob_id: &str,
    peer_device_id: &str,
    session_id: u64,
    framed: Framed<TlsStream<TcpStream>, LengthDelimitedCodec>,
) -> mpsc::Sender<SessionCommand> {
    let (command_tx, command_rx) = mpsc::channel(128);
    let disconnect_tx = engine_control_tx.clone();
    let peer_noob_id_for_task = peer_noob_id.to_string();
    let peer_device_id_for_task = peer_device_id.to_string();

    let actor_config = config.clone();
    let actor_event_tx = event_tx.clone();
    let actor_progress_tx = progress_tx.clone();
    let actor_shutdown_tx = shutdown_tx.clone();

    tokio::spawn(async move {
        let actor_result = run_session_actor(SessionActorContext {
            peer_noob_id: peer_noob_id_for_task.clone(),
            peer_device_id: peer_device_id_for_task.clone(),
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
                    peer_noob_id: peer_noob_id_for_task.clone(),
                    session_id,
                    error,
                })
                .await;
        }

        let _ = disconnect_tx
            .send(EngineControl::PeerDisconnected {
                peer_noob_id: peer_noob_id_for_task,
                session_id,
            })
            .await;
    });

    command_tx
}

fn emit_connection_error_event(
    event_tx: &mpsc::Sender<SyncEvent>,
    peer_noob_id: Option<String>,
    addr: Option<std::net::SocketAddr>,
    error: SyncError,
) {
    if let Err(send_error) = event_tx.try_send(SyncEvent::ConnectionError {
        peer_noob_id,
        addr,
        error: error.to_string(),
    }) {
        warn!("drop connection error event: {send_error}");
    }
}

fn publish_peer_snapshot(
    registry: &PeerRegistry,
    peers_tx: &watch::Sender<Vec<ConnectedPeerInfo>>,
) {
    let _ = peers_tx.send(registry.snapshot());
}

fn now_millis_u64() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    if millis > u64::MAX as u128 {
        u64::MAX
    } else {
        millis as u64
    }
}
