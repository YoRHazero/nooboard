use tokio::spawn;
use tokio::sync::{broadcast, mpsc};

use super::tasks::{connect_outbound, now_millis};
use super::{InternalEvent, ReadySession, RunningContext, RuntimeManager};
use crate::connection::coordinator::BeginAttempt;
use crate::errors::NetworkResult;
use crate::lan::runtime::{LanRuntimeConfig, LanRuntimeEvent, spawn_lan_runtime};
use crate::{ConnectionFailure, ConnectionFailureKind, ConnectionMode, NetworkEvent};

impl RuntimeManager {
    pub(super) async fn ensure_lan_running(&self, running: &RunningContext) -> NetworkResult<()> {
        let mut driver = self.inner.driver.lock().await;
        if driver.lan_task.is_some() {
            return Ok(());
        }

        let lan_config = LanRuntimeConfig {
            local_noob_id: self.inner.startup.identity.noob_id.clone(),
            local_device_id: self.inner.startup.identity.device_id.clone(),
            boot_id: running.boot_id.clone(),
            listen_port: self.inner.startup.listen_port,
            advertise_addrs: running.listener_bindings.advertise_addrs.clone(),
        };
        let (lan_tx, mut lan_rx) = mpsc::channel(64);
        let internal_tx = running.internal_tx.clone();
        let lan_bridge_task = spawn(async move {
            while let Some(event) = lan_rx.recv().await {
                if internal_tx.send(InternalEvent::Lan(event)).await.is_err() {
                    break;
                }
            }
        });
        let (lan_shutdown_tx, mut lan_shutdown_rx) = broadcast::channel(1);
        let internal_tx = running.internal_tx.clone();
        let lan_task = spawn(async move {
            let handle = match spawn_lan_runtime(lan_config, lan_tx) {
                Ok(handle) => handle,
                Err(error) => {
                    let _ = internal_tx
                        .send(InternalEvent::ConnectFailed(ConnectionFailure {
                            kind: ConnectionFailureKind::Internal,
                            mode: ConnectionMode::Lan,
                            peer_noob_id: None,
                            peer_device_id: None,
                            remote_addr: None,
                            local_bind_addr: None,
                            detail: error.to_string(),
                        }))
                        .await;
                    return;
                }
            };
            let _ = lan_shutdown_rx.recv().await;
            handle.shutdown().await;
        });
        driver.lan_shutdown_tx = Some(lan_shutdown_tx);
        driver.lan_task = Some(lan_task);
        driver.lan_bridge_task = Some(lan_bridge_task);
        Ok(())
    }

    pub(super) async fn stop_lan_runtime(&self) {
        let (lan_shutdown_tx, lan_task, lan_bridge_task) = {
            let mut driver = self.inner.driver.lock().await;
            (
                driver.lan_shutdown_tx.take(),
                driver.lan_task.take(),
                driver.lan_bridge_task.take(),
            )
        };
        if let Some(lan_shutdown_tx) = lan_shutdown_tx {
            let _ = lan_shutdown_tx.send(());
        }
        if let Some(lan_task) = lan_task {
            let _ = lan_task.await;
        }
        if let Some(lan_bridge_task) = lan_bridge_task {
            let _ = lan_bridge_task.await;
        }
    }

    pub(super) async fn handle_lan_event(&self, event: LanRuntimeEvent) {
        match event {
            LanRuntimeEvent::Resolved(record) => {
                {
                    let mut state = self.inner.state.lock().await;
                    state.apply_lan_service_resolved(record);
                    self.replace_snapshot(state.snapshot());
                }
                self.inner.event_hub.publish(NetworkEvent::LanPeersChanged);
                self.schedule_lan_connects().await;
            }
            LanRuntimeEvent::Removed(fullname) => {
                let changed = {
                    let mut state = self.inner.state.lock().await;
                    let changed = state.apply_lan_service_removed(&fullname);
                    if changed {
                        self.replace_snapshot(state.snapshot());
                    }
                    changed
                };
                if changed {
                    self.inner.event_hub.publish(NetworkEvent::LanPeersChanged);
                }
            }
        }
    }

    pub(super) async fn schedule_lan_connects(&self) {
        let running = match self.running_context().await {
            Ok(running) => running,
            Err(_) => return,
        };

        loop {
            let candidate = {
                let mut state = self.inner.state.lock().await;
                let candidate =
                    state.next_lan_candidate(&self.inner.startup.identity.noob_id, now_millis());
                if let Some(candidate) = &candidate {
                    state.mark_lan_connecting(&candidate.noob_id);
                }
                candidate
            };
            let Some(candidate) = candidate else {
                break;
            };

            let manager = self.clone();
            let addr = candidate.addr;
            let peer_noob_id = candidate.noob_id.clone();
            let running = running.clone();
            let task = spawn(async move {
                run_lan_connect_task(manager, running, candidate).await;
            });

            let mut driver = self.inner.driver.lock().await;
            match driver.coordinator.begin(ConnectionMode::Lan, addr, task) {
                BeginAttempt::Granted => {}
                BeginAttempt::Skipped => {
                    let mut state = self.inner.state.lock().await;
                    state.note_lan_connect_finished(&peer_noob_id);
                }
            }
        }
    }
}

pub(super) async fn run_lan_connect_task(
    manager: RuntimeManager,
    context: RunningContext,
    candidate: crate::lan::peer_index::LanConnectCandidate,
) {
    let result = connect_outbound(
        manager.inner.startup.runtime_config(),
        &context.tls,
        crate::protocol::ConnectionIntent::LanSync,
        candidate.addr,
    )
    .await;

    match result {
        Ok((peer, local_bind_addr, framed)) => {
            let _ = context
                .internal_tx
                .send(InternalEvent::SessionReady(ReadySession {
                    mode: ConnectionMode::Lan,
                    seed_id: None,
                    remote_addr: candidate.addr,
                    local_bind_addr: Some(local_bind_addr),
                    peer_noob_id: peer.noob_id,
                    peer_device_id: peer.device_id,
                    outbound: true,
                    framed,
                }))
                .await;
        }
        Err(mut failure) => {
            failure.mode = ConnectionMode::Lan;
            failure.peer_noob_id = Some(candidate.noob_id.clone());
            failure.peer_device_id = Some(candidate.device_id.clone());
            let _ = context
                .internal_tx
                .send(InternalEvent::ConnectFailed(failure))
                .await;
        }
    }

    manager
        .finish_outbound_attempt(ConnectionMode::Lan, candidate.addr)
        .await;
    {
        let mut state = manager.inner.state.lock().await;
        state.note_lan_connect_finished(&candidate.noob_id);
    }
}
