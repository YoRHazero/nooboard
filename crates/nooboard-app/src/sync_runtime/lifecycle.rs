use std::time::Duration;

use nooboard_sync::error::ConnectionError;
use nooboard_sync::{ConnectedPeerInfo, SyncConfig, SyncError, SyncStatus, start_sync_engine};
use tokio::time::timeout;

use crate::{AppError, AppResult};

use super::SyncRuntime;
use super::bridge::{
    abort_bridge_task, spawn_event_bridge, spawn_peer_bridge, spawn_status_bridge,
    spawn_transfer_bridge, wait_for_engine_termination,
};
use super::state::RunningEngine;

const ENGINE_STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(3);
const ENGINE_START_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

impl SyncRuntime {
    pub fn mark_disabled(&mut self) {
        let _ = self.state.peers_tx.send(Vec::new());
        let _ = self.state.status_tx.send(SyncStatus::Disabled);
    }

    pub async fn start(&mut self, config: SyncConfig) -> AppResult<()> {
        if self.state.engine.is_some() {
            return Ok(());
        }

        if !config.enabled {
            self.mark_disabled();
            return Ok(());
        }

        let _ = self.state.status_tx.send(SyncStatus::Starting);

        let handle = start_sync_engine(config).await?;
        let nooboard_sync::SyncEngineHandle {
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
        } = handle;

        let mut startup_status_rx = status_rx.clone();

        let event_task = spawn_event_bridge(event_rx, self.state.event_tx.clone());
        let transfer_task = spawn_transfer_bridge(progress_rx, self.state.transfer_tx.clone());
        let peer_task = spawn_peer_bridge(peers_rx.clone(), self.state.peers_tx.clone());
        let status_task = spawn_status_bridge(status_rx.clone(), self.state.status_tx.clone());

        let running_engine = RunningEngine {
            text_tx,
            file_tx,
            decision_tx,
            control_tx,
            status_rx,
            shutdown_tx,
            engine_task,
            event_task,
            transfer_task,
            peer_task,
            status_task,
        };

        if let Err(startup_error) =
            wait_for_engine_startup(&mut startup_status_rx, ENGINE_START_WAIT_TIMEOUT).await
        {
            shutdown_engine(running_engine).await;
            return Err(startup_error);
        }

        self.state.engine = Some(running_engine);
        Ok(())
    }

    pub async fn stop(&mut self) -> AppResult<()> {
        if let Some(engine) = self.state.engine.take() {
            shutdown_engine(engine).await;
        } else {
            let _ = self.state.peers_tx.send(Vec::new());
            let _ = self.state.status_tx.send(SyncStatus::Stopped);
        }
        Ok(())
    }

    pub async fn restart(&mut self, config: SyncConfig) -> AppResult<()> {
        self.stop().await?;
        self.start(config).await
    }

    pub fn status(&self) -> SyncStatus {
        self.state.status_tx.borrow().clone()
    }

    pub fn connected_peers(&self) -> Vec<ConnectedPeerInfo> {
        self.state.peers_tx.borrow().clone()
    }
}

async fn shutdown_engine(mut engine: RunningEngine) {
    let _ = engine.shutdown_tx.send(());
    wait_for_engine_termination(&mut engine.status_rx, ENGINE_STOP_WAIT_TIMEOUT).await;
    if let Some(mut engine_task) = engine.engine_task.take() {
        if timeout(ENGINE_STOP_WAIT_TIMEOUT, &mut engine_task)
            .await
            .is_err()
        {
            engine_task.abort();
            let _ = engine_task.await;
        }
    }
    abort_bridge_task(engine.event_task).await;
    abort_bridge_task(engine.transfer_task).await;
    abort_bridge_task(engine.peer_task).await;
    abort_bridge_task(engine.status_task).await;
}

async fn wait_for_engine_startup(
    status_rx: &mut tokio::sync::watch::Receiver<SyncStatus>,
    max_wait: Duration,
) -> AppResult<()> {
    let wait = async {
        loop {
            match status_rx.borrow().clone() {
                SyncStatus::Running | SyncStatus::Disabled => return Ok(()),
                SyncStatus::Error(message) => {
                    return Err(AppError::Sync(SyncError::Connection(
                        ConnectionError::State(message),
                    )));
                }
                SyncStatus::Stopped => {
                    return Err(AppError::Sync(SyncError::ChannelClosed));
                }
                SyncStatus::Starting => {}
            }

            if status_rx.changed().await.is_err() {
                return Err(AppError::Sync(SyncError::ChannelClosed));
            }
        }
    };

    match timeout(max_wait, wait).await {
        Ok(result) => result,
        Err(_) => Err(AppError::Sync(SyncError::Connection(
            ConnectionError::State("sync engine start timed out".to_string()),
        ))),
    }
}
