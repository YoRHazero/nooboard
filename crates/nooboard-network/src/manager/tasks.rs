use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{MissedTickBehavior, interval, timeout};

use super::{InboundDirectPending, InternalEvent, ReadySession, RunningContext, RuntimeManager};
use crate::auth::{ChallengeRegistry, SocketId};
use crate::config::NetworkConfig;
use crate::connection::handshake::{
    AuthenticatedPeer, perform_client_handshake, perform_server_handshake,
};
use crate::errors::{ConnectionError, NetworkError, NetworkResult, TransportError};
use crate::listener::bindings::ListenerBindings;
use crate::listener::source_select::select_local_ipv4_for_remote;
use crate::protocol::ConnectionIntent;
use crate::session::actor::SessionLifecycleEvent;
use crate::transport::{TlsContext, framed_with_max_packet};
use crate::{ConnectionFailure, ConnectionFailureKind, ConnectionMode};

impl RuntimeManager {
    pub(super) async fn start_runtime_tasks(&self) -> NetworkResult<()> {
        let (listener_bindings, listener) =
            ListenerBindings::bind(self.inner.startup.listen_port).await?;
        let tls =
            TlsContext::ephemeral().map_err(|error| NetworkError::Internal(error.to_string()))?;
        let challenge_registry = Arc::new(ChallengeRegistry::new());
        let boot_id = uuid::Uuid::now_v7().to_string();
        let (shutdown_tx, _) = broadcast::channel(8);
        let (internal_tx, internal_rx) = mpsc::channel(super::INTERNAL_EVENT_CAPACITY);
        let (lifecycle_tx, lifecycle_rx) = mpsc::channel(super::SESSION_LIFECYCLE_CAPACITY);

        let manager = self.clone();
        let internal_task = tokio::spawn(run_internal_loop(
            manager.clone(),
            internal_rx,
            shutdown_tx.subscribe(),
        ));

        let internal_tx_for_lifecycle = internal_tx.clone();
        let lifecycle_task = tokio::spawn(async move {
            run_lifecycle_bridge(lifecycle_rx, internal_tx_for_lifecycle).await;
        });

        let accept_task = tokio::spawn(run_accept_loop(
            self.inner.startup.clone(),
            listener,
            tls.clone(),
            challenge_registry.clone(),
            internal_tx.clone(),
            shutdown_tx.subscribe(),
            Arc::new(AtomicU64::new(1)),
        ));

        let maintenance_task = tokio::spawn(run_maintenance_loop(
            challenge_registry,
            internal_tx.clone(),
            shutdown_tx.subscribe(),
        ));

        {
            let mut driver = self.inner.driver.lock().await;
            driver.boot_id = Some(boot_id.clone());
            driver.listener_bindings = Some(listener_bindings.clone());
            driver.tls = Some(tls.clone());
            driver.shutdown_tx = Some(shutdown_tx.clone());
            driver.internal_tx = Some(internal_tx.clone());
            driver.lifecycle_tx = Some(lifecycle_tx);
            driver.lifecycle_task = Some(lifecycle_task);
            driver.internal_task = Some(internal_task);
            driver.accept_task = Some(accept_task);
            driver.maintenance_task = Some(maintenance_task);
        }

        let lan_enabled = {
            let state = self.inner.state.lock().await;
            state.snapshot().lan_enabled
        };
        if lan_enabled {
            let lifecycle_tx = {
                let driver = self.inner.driver.lock().await;
                driver.lifecycle_tx.clone().expect("lifecycle tx")
            };
            let running = RunningContext {
                tls,
                shutdown_tx,
                internal_tx,
                lifecycle_tx,
                boot_id,
                listener_bindings,
            };
            self.ensure_lan_running(&running).await?;
        }

        Ok(())
    }

    pub(super) async fn running_context(&self) -> NetworkResult<RunningContext> {
        let driver = self.inner.driver.lock().await;
        let Some(tls) = driver.tls.clone() else {
            return Err(NetworkError::NotRunning);
        };
        let Some(shutdown_tx) = driver.shutdown_tx.clone() else {
            return Err(NetworkError::NotRunning);
        };
        let Some(internal_tx) = driver.internal_tx.clone() else {
            return Err(NetworkError::NotRunning);
        };
        let Some(lifecycle_tx) = driver.lifecycle_tx.clone() else {
            return Err(NetworkError::NotRunning);
        };
        let Some(boot_id) = driver.boot_id.clone() else {
            return Err(NetworkError::NotRunning);
        };
        let Some(listener_bindings) = driver.listener_bindings.clone() else {
            return Err(NetworkError::NotRunning);
        };
        Ok(RunningContext {
            tls,
            shutdown_tx,
            internal_tx,
            lifecycle_tx,
            boot_id,
            listener_bindings,
        })
    }

    pub(super) async fn finish_outbound_attempt(&self, mode: ConnectionMode, addr: SocketAddr) {
        let mut driver = self.inner.driver.lock().await;
        driver.coordinator.finish(mode, addr);
    }
}

pub(super) async fn run_internal_loop(
    manager: RuntimeManager,
    mut rx: mpsc::Receiver<InternalEvent>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            maybe_event = rx.recv() => {
                let Some(event) = maybe_event else {
                    break;
                };
                manager.handle_internal_event(event).await;
            }
        }
    }
}

async fn run_lifecycle_bridge(
    mut rx: mpsc::Receiver<SessionLifecycleEvent>,
    internal_tx: mpsc::Sender<InternalEvent>,
) {
    while let Some(event) = rx.recv().await {
        if internal_tx
            .send(InternalEvent::SessionLifecycle(event))
            .await
            .is_err()
        {
            break;
        }
    }
}

async fn run_maintenance_loop(
    challenge_registry: Arc<ChallengeRegistry>,
    internal_tx: mpsc::Sender<InternalEvent>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            _ = ticker.tick() => {
                challenge_registry.prune_expired().await;
                if internal_tx.send(InternalEvent::MaintenanceTick).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn run_accept_loop(
    startup: super::RuntimeStartup,
    listener: TcpListener,
    tls: TlsContext,
    challenge_registry: Arc<ChallengeRegistry>,
    internal_tx: mpsc::Sender<InternalEvent>,
    mut shutdown_rx: broadcast::Receiver<()>,
    socket_counter: Arc<AtomicU64>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            accepted = listener.accept() => {
                let Ok((stream, remote_addr)) = accepted else {
                    continue;
                };
                let local_bind_addr = stream.local_addr().ok();
                let startup = startup.clone();
                let tls = tls.clone();
                let challenge_registry = challenge_registry.clone();
                let internal_tx = internal_tx.clone();
                let socket_id = socket_counter.fetch_add(1, Ordering::Relaxed) as SocketId;

                tokio::spawn(async move {
                    let config = startup.runtime_config();
                    let result = accept_inbound_connection(
                        config,
                        stream,
                        tls,
                        challenge_registry,
                        socket_id,
                    )
                    .await;
                    match result {
                        Ok((peer, framed)) => {
                            let event = match peer.intent {
                                ConnectionIntent::LanSync => InternalEvent::SessionReady(ReadySession {
                                    mode: ConnectionMode::Lan,
                                    seed_id: None,
                                    remote_addr,
                                    local_bind_addr,
                                    peer_noob_id: peer.noob_id,
                                    peer_device_id: peer.device_id,
                                    outbound: false,
                                    framed,
                                }),
                                ConnectionIntent::DirectConnect => InternalEvent::InboundDirect(InboundDirectPending {
                                    remote_addr,
                                    local_bind_addr,
                                    peer_noob_id: peer.noob_id,
                                    peer_device_id: peer.device_id,
                                    framed,
                                }),
                            };
                            let _ = internal_tx.send(event).await;
                        }
                        Err(failure) => {
                            let _ = internal_tx.send(InternalEvent::ConnectFailed(failure)).await;
                        }
                    }
                });
            }
        }
    }
}

async fn accept_inbound_connection(
    config: NetworkConfig,
    stream: TcpStream,
    tls: TlsContext,
    challenge_registry: Arc<ChallengeRegistry>,
    socket_id: SocketId,
) -> Result<(AuthenticatedPeer, crate::transport::NetworkFramed), ConnectionFailure> {
    let remote_addr = stream.peer_addr().ok();
    let local_bind_addr = stream.local_addr().ok();
    let tls_stream = tls
        .accept(stream)
        .await
        .map_err(|error| ConnectionFailure {
            kind: ConnectionFailureKind::TlsHandshakeFailed,
            mode: ConnectionMode::Direct,
            peer_noob_id: None,
            peer_device_id: None,
            remote_addr,
            local_bind_addr,
            detail: error.to_string(),
        })?;
    let mut framed = framed_with_max_packet(tls_stream, config.transport.max_packet_size);
    let peer = perform_server_handshake(&config, socket_id, &challenge_registry, &mut framed)
        .await
        .map_err(|error| ConnectionFailure {
            kind: classify_connection_error(&error),
            mode: ConnectionMode::Direct,
            peer_noob_id: None,
            peer_device_id: None,
            remote_addr,
            local_bind_addr,
            detail: error.to_string(),
        })?;
    Ok((peer, framed))
}

pub(super) async fn connect_outbound(
    config: NetworkConfig,
    tls: &TlsContext,
    intent: ConnectionIntent,
    remote_addr: SocketAddr,
) -> Result<
    (
        AuthenticatedPeer,
        SocketAddr,
        crate::transport::NetworkFramed,
    ),
    ConnectionFailure,
> {
    let dialed = dial_ipv4(remote_addr, config.transport.connect_timeout_ms).await?;
    let tls_stream = tls
        .connect(dialed.stream, "nooboard.local")
        .await
        .map_err(|error| ConnectionFailure {
            kind: ConnectionFailureKind::TlsHandshakeFailed,
            mode: match intent {
                ConnectionIntent::LanSync => ConnectionMode::Lan,
                ConnectionIntent::DirectConnect => ConnectionMode::Direct,
            },
            peer_noob_id: None,
            peer_device_id: None,
            remote_addr: Some(remote_addr),
            local_bind_addr: Some(dialed.local_addr),
            detail: error.to_string(),
        })?;
    let mut framed = framed_with_max_packet(tls_stream, config.transport.max_packet_size);
    let peer = perform_client_handshake(&config, remote_addr, &mut framed, intent)
        .await
        .map_err(|error| ConnectionFailure {
            kind: classify_connection_error(&error),
            mode: match intent {
                ConnectionIntent::LanSync => ConnectionMode::Lan,
                ConnectionIntent::DirectConnect => ConnectionMode::Direct,
            },
            peer_noob_id: None,
            peer_device_id: None,
            remote_addr: Some(remote_addr),
            local_bind_addr: Some(dialed.local_addr),
            detail: error.to_string(),
        })?;
    Ok((peer, dialed.local_addr, framed))
}

struct DialedStream {
    stream: TcpStream,
    local_addr: SocketAddr,
}

async fn dial_ipv4(
    remote_addr: SocketAddr,
    connect_timeout_ms: u64,
) -> Result<DialedStream, ConnectionFailure> {
    let local_ip =
        select_local_ipv4_for_remote(remote_addr).map_err(|error| ConnectionFailure {
            kind: ConnectionFailureKind::ConnectFailed,
            mode: ConnectionMode::Direct,
            peer_noob_id: None,
            peer_device_id: None,
            remote_addr: Some(remote_addr),
            local_bind_addr: None,
            detail: error.to_string(),
        })?;
    let socket = TcpSocket::new_v4().map_err(|error| ConnectionFailure {
        kind: ConnectionFailureKind::ConnectFailed,
        mode: ConnectionMode::Direct,
        peer_noob_id: None,
        peer_device_id: None,
        remote_addr: Some(remote_addr),
        local_bind_addr: None,
        detail: error.to_string(),
    })?;
    let local_addr = SocketAddr::new(IpAddr::V4(local_ip), 0);
    socket.bind(local_addr).map_err(|error| ConnectionFailure {
        kind: ConnectionFailureKind::ConnectFailed,
        mode: ConnectionMode::Direct,
        peer_noob_id: None,
        peer_device_id: None,
        remote_addr: Some(remote_addr),
        local_bind_addr: Some(local_addr),
        detail: error.to_string(),
    })?;
    let stream = timeout(
        Duration::from_millis(connect_timeout_ms),
        socket.connect(remote_addr),
    )
    .await
    .map_err(|_| ConnectionFailure {
        kind: ConnectionFailureKind::ConnectFailed,
        mode: ConnectionMode::Direct,
        peer_noob_id: None,
        peer_device_id: None,
        remote_addr: Some(remote_addr),
        local_bind_addr: Some(local_addr),
        detail: "connect timeout".to_string(),
    })?
    .map_err(|error| ConnectionFailure {
        kind: ConnectionFailureKind::ConnectFailed,
        mode: ConnectionMode::Direct,
        peer_noob_id: None,
        peer_device_id: None,
        remote_addr: Some(remote_addr),
        local_bind_addr: Some(local_addr),
        detail: error.to_string(),
    })?;
    let bound_addr = stream.local_addr().unwrap_or(local_addr);
    Ok(DialedStream {
        stream,
        local_addr: bound_addr,
    })
}

pub(super) fn classify_transport_error(error: &TransportError) -> ConnectionFailureKind {
    match error {
        TransportError::Rustls(_) => ConnectionFailureKind::TlsHandshakeFailed,
        TransportError::Protocol(_) => ConnectionFailureKind::ProtocolMismatch,
        TransportError::Io(_) => ConnectionFailureKind::Io,
        TransportError::InvalidServerName(_) => ConnectionFailureKind::Internal,
    }
}

pub(super) fn classify_connection_error(error: &ConnectionError) -> ConnectionFailureKind {
    match error {
        ConnectionError::Transport(transport) => classify_transport_error(transport),
        ConnectionError::FileReceive(_) => ConnectionFailureKind::Internal,
        ConnectionError::PongTimeout => ConnectionFailureKind::Io,
        ConnectionError::Io(_) => ConnectionFailureKind::Io,
        ConnectionError::State(detail) => {
            if detail.contains("protocol version mismatch") {
                ConnectionFailureKind::ProtocolMismatch
            } else if detail.contains("auth rejected") || detail.contains("authentication failed") {
                ConnectionFailureKind::AuthRejected
            } else {
                ConnectionFailureKind::Internal
            }
        }
    }
}

pub(super) fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
