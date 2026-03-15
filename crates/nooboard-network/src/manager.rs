use std::sync::{Arc, RwLock};

use tokio::sync::{Mutex, broadcast, mpsc};
use tokio::task::JoinHandle;

use crate::config::{
    DirectConfig, LanConfig, LocalIdentityConfig, NetworkAuthConfig, NetworkConfig,
    NetworkTransferConfig, NetworkTransportConfig,
};
use crate::connection::coordinator::ConnectionCoordinator;
use crate::errors::{NetworkError, NetworkResult};
use crate::event_hub::EventHub;
use crate::lan::runtime::LanRuntimeEvent;
use crate::listener::bindings::ListenerBindings;
use crate::session::actor::SessionLifecycleEvent;
use crate::state::RuntimeState;
use crate::transport::TlsContext;
use crate::{
    ConnectionFailure, ConnectionMode, DirectRequestId, DirectSeedId, NetworkEvent,
    NetworkSnapshot, NetworkSubscription,
};

#[path = "manager/direct.rs"]
mod direct;
#[path = "manager/lan.rs"]
mod lan;
#[path = "manager/session.rs"]
mod session;
#[path = "manager/tasks.rs"]
mod tasks;

const INTERNAL_EVENT_CAPACITY: usize = 256;
const SESSION_LIFECYCLE_CAPACITY: usize = 256;

#[derive(Clone)]
pub(crate) struct RuntimeManager {
    inner: Arc<ManagerInner>,
}

struct ManagerInner {
    startup: RuntimeStartup,
    state: Mutex<RuntimeState>,
    snapshot: RwLock<NetworkSnapshot>,
    event_hub: EventHub,
    driver: Mutex<RuntimeDriver>,
}

#[derive(Clone)]
struct RuntimeStartup {
    identity: LocalIdentityConfig,
    listen_port: u16,
    auth: NetworkAuthConfig,
    transport: NetworkTransportConfig,
    transfer: NetworkTransferConfig,
    approval_timeout_ms: u64,
}

struct RuntimeDriver {
    boot_id: Option<String>,
    listener_bindings: Option<ListenerBindings>,
    tls: Option<TlsContext>,
    shutdown_tx: Option<broadcast::Sender<()>>,
    internal_tx: Option<mpsc::Sender<InternalEvent>>,
    lifecycle_tx: Option<mpsc::Sender<SessionLifecycleEvent>>,
    lifecycle_task: Option<JoinHandle<()>>,
    internal_task: Option<JoinHandle<()>>,
    accept_task: Option<JoinHandle<()>>,
    maintenance_task: Option<JoinHandle<()>>,
    lan_shutdown_tx: Option<broadcast::Sender<()>>,
    lan_task: Option<JoinHandle<()>>,
    lan_bridge_task: Option<JoinHandle<()>>,
    coordinator: ConnectionCoordinator,
}

impl Default for RuntimeDriver {
    fn default() -> Self {
        Self {
            boot_id: None,
            listener_bindings: None,
            tls: None,
            shutdown_tx: None,
            internal_tx: None,
            lifecycle_tx: None,
            lifecycle_task: None,
            internal_task: None,
            accept_task: None,
            maintenance_task: None,
            lan_shutdown_tx: None,
            lan_task: None,
            lan_bridge_task: None,
            coordinator: ConnectionCoordinator::default(),
        }
    }
}

enum InternalEvent {
    SessionLifecycle(SessionLifecycleEvent),
    Lan(LanRuntimeEvent),
    SessionReady(ReadySession),
    InboundDirect(InboundDirectPending),
    ConnectFailed(ConnectionFailure),
    ApprovalExpired(DirectRequestId),
    MaintenanceTick,
}

struct ReadySession {
    mode: ConnectionMode,
    seed_id: Option<DirectSeedId>,
    remote_addr: std::net::SocketAddr,
    local_bind_addr: Option<std::net::SocketAddr>,
    peer_noob_id: String,
    peer_device_id: String,
    outbound: bool,
    framed: crate::transport::NetworkFramed,
}

struct InboundDirectPending {
    remote_addr: std::net::SocketAddr,
    local_bind_addr: Option<std::net::SocketAddr>,
    peer_noob_id: String,
    peer_device_id: String,
    framed: crate::transport::NetworkFramed,
}

#[derive(Clone)]
struct RunningContext {
    tls: TlsContext,
    shutdown_tx: broadcast::Sender<()>,
    internal_tx: mpsc::Sender<InternalEvent>,
    lifecycle_tx: mpsc::Sender<SessionLifecycleEvent>,
    boot_id: String,
    listener_bindings: ListenerBindings,
}

impl RuntimeStartup {
    fn from_config(config: &NetworkConfig) -> Self {
        Self {
            identity: config.identity.clone(),
            listen_port: config.listen_port,
            auth: config.auth.clone(),
            transport: config.transport.clone(),
            transfer: config.transfer.clone(),
            approval_timeout_ms: config.direct.approval_timeout_ms,
        }
    }

    fn runtime_config(&self) -> NetworkConfig {
        NetworkConfig {
            identity: self.identity.clone(),
            listen_port: self.listen_port,
            auth: self.auth.clone(),
            lan: LanConfig { enabled: false },
            direct: DirectConfig {
                approval_timeout_ms: self.approval_timeout_ms,
                seeds: Vec::new(),
            },
            transport: self.transport.clone(),
            transfer: self.transfer.clone(),
        }
    }
}

impl RuntimeManager {
    pub(crate) fn new(config: NetworkConfig) -> NetworkResult<Self> {
        config.validate()?;
        let state = RuntimeState::new(&config);
        let snapshot = state.snapshot();
        Ok(Self {
            inner: Arc::new(ManagerInner {
                startup: RuntimeStartup::from_config(&config),
                state: Mutex::new(state),
                snapshot: RwLock::new(snapshot),
                event_hub: EventHub::new(),
                driver: Mutex::new(RuntimeDriver::default()),
            }),
        })
    }

    pub(crate) async fn start(&self) -> NetworkResult<()> {
        {
            let driver = self.inner.driver.lock().await;
            if driver.shutdown_tx.is_some() {
                return Ok(());
            }
        }

        let start_events = {
            let mut state = self.inner.state.lock().await;
            let events = state.start();
            self.replace_snapshot(state.snapshot());
            events
        };
        self.inner.event_hub.publish_all(start_events);

        let start_result = self.start_runtime_tasks().await;
        if let Err(error) = start_result {
            let error_events = {
                let mut state = self.inner.state.lock().await;
                let events = state.set_error(error.to_string());
                self.replace_snapshot(state.snapshot());
                events
            };
            self.inner.event_hub.publish_all(error_events);
            return Err(error);
        }

        let running_events = {
            let mut state = self.inner.state.lock().await;
            let events = state.mark_running();
            self.replace_snapshot(state.snapshot());
            events
        };
        self.inner.event_hub.publish_all(running_events);
        self.schedule_lan_connects().await;
        Ok(())
    }

    pub(crate) async fn shutdown(&self) -> NetworkResult<()> {
        let (
            shutdown_tx,
            internal_tx,
            lifecycle_tx,
            lifecycle_task,
            internal_task,
            accept_task,
            maintenance_task,
            lan_shutdown_tx,
            lan_task,
            lan_bridge_task,
        ) = {
            let mut driver = self.inner.driver.lock().await;
            driver.coordinator.abort_all();
            (
                driver.shutdown_tx.take(),
                driver.internal_tx.take(),
                driver.lifecycle_tx.take(),
                driver.lifecycle_task.take(),
                driver.internal_task.take(),
                driver.accept_task.take(),
                driver.maintenance_task.take(),
                driver.lan_shutdown_tx.take(),
                driver.lan_task.take(),
                driver.lan_bridge_task.take(),
            )
        };

        {
            let state = self.inner.state.lock().await;
            state.shutdown_all_sessions();
        }

        if let Some(shutdown_tx) = shutdown_tx {
            let _ = shutdown_tx.send(());
        }
        drop(internal_tx);
        drop(lifecycle_tx);
        if let Some(lan_shutdown_tx) = lan_shutdown_tx {
            let _ = lan_shutdown_tx.send(());
        }
        if let Some(lan_task) = lan_task {
            let _ = lan_task.await;
        }
        if let Some(lan_bridge_task) = lan_bridge_task {
            let _ = lan_bridge_task.await;
        }
        if let Some(accept_task) = accept_task {
            let _ = accept_task.await;
        }
        if let Some(maintenance_task) = maintenance_task {
            let _ = maintenance_task.await;
        }
        if let Some(lifecycle_task) = lifecycle_task {
            let _ = lifecycle_task.await;
        }
        if let Some(internal_task) = internal_task {
            let _ = internal_task.await;
        }

        {
            let mut driver = self.inner.driver.lock().await;
            driver.boot_id = None;
            driver.listener_bindings = None;
            driver.tls = None;
        }

        let events = {
            let mut state = self.inner.state.lock().await;
            let events = state.shutdown();
            self.replace_snapshot(state.snapshot());
            events
        };
        self.inner.event_hub.publish_all(events);
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> NetworkSnapshot {
        self.inner
            .snapshot
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(crate) fn subscribe(&self) -> NetworkSubscription {
        self.inner.event_hub.subscribe()
    }

    pub(crate) async fn set_lan_enabled(&self, enabled: bool) -> NetworkResult<()> {
        let events = {
            let mut state = self.inner.state.lock().await;
            let events = state.set_lan_enabled(enabled);
            self.replace_snapshot(state.snapshot());
            events
        };
        self.inner.event_hub.publish_all(events);

        let running = match self.running_context().await {
            Ok(running) => running,
            Err(NetworkError::NotRunning) => return Ok(()),
            Err(error) => return Err(error),
        };
        if enabled {
            self.ensure_lan_running(&running).await?;
            self.schedule_lan_connects().await;
        } else {
            self.stop_lan_runtime().await;
        }
        Ok(())
    }

    async fn handle_internal_event(&self, event: InternalEvent) {
        match event {
            InternalEvent::SessionLifecycle(event) => self.handle_session_lifecycle(event).await,
            InternalEvent::Lan(event) => self.handle_lan_event(event).await,
            InternalEvent::SessionReady(ready) => self.activate_session(ready).await,
            InternalEvent::InboundDirect(pending) => self.handle_inbound_direct(pending).await,
            InternalEvent::ConnectFailed(failure) => {
                if failure.mode == ConnectionMode::Lan
                    && let Some(peer_noob_id) = &failure.peer_noob_id
                {
                    let mut state = self.inner.state.lock().await;
                    state.note_lan_connect_failure(peer_noob_id, tasks::now_millis());
                }
                self.inner
                    .event_hub
                    .publish(NetworkEvent::ConnectionFailed(failure));
            }
            InternalEvent::ApprovalExpired(id) => self.expire_direct_request(id).await,
            InternalEvent::MaintenanceTick => {
                self.schedule_lan_connects().await;
            }
        }
    }
}

impl RuntimeManager {
    pub(crate) fn replace_snapshot(&self, snapshot: NetworkSnapshot) {
        *self
            .inner
            .snapshot
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = snapshot;
    }

    #[cfg(test)]
    pub(crate) fn set_publish_hook(
        &self,
        hook: std::sync::Arc<dyn Fn(&NetworkEvent) + Send + Sync>,
    ) {
        self.inner.event_hub.set_before_send_hook(hook);
    }
}
