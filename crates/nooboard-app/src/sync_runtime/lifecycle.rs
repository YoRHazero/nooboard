use std::time::Duration;

use nooboard_sync::error::ConnectionError;
use nooboard_sync::{ConnectedPeerInfo, SyncConfig, SyncError, SyncStatus, start_sync_engine};
use tokio::time::timeout;

use crate::{AppError, AppResult};

use super::SyncRuntime;
use super::bridge::{
    abort_bridge_task, spawn_event_bridge, spawn_transfer_bridge, wait_for_engine_termination,
};
use super::state::RunningEngine;

const ENGINE_STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(3);
const ENGINE_START_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

impl SyncRuntime {
    pub async fn start(&mut self, config: SyncConfig) -> AppResult<()> {
        if self.state.engine.is_some() {
            return Ok(());
        }

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
        } = handle;

        let mut startup_status_rx = status_rx.clone();

        let event_task = spawn_event_bridge(event_rx, self.state.event_tx.clone());
        let transfer_task = spawn_transfer_bridge(progress_rx, self.state.transfer_tx.clone());

        let running_engine = RunningEngine {
            text_tx,
            file_tx,
            decision_tx,
            control_tx,
            peers_rx,
            status_rx,
            shutdown_tx,
            event_task,
            transfer_task,
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
        }
        Ok(())
    }

    pub async fn restart(&mut self, config: SyncConfig) -> AppResult<()> {
        self.stop().await?;
        self.start(config).await
    }

    pub fn status(&self) -> SyncStatus {
        self.state
            .engine
            .as_ref()
            .map(|engine| engine.status_rx.borrow().clone())
            .unwrap_or(SyncStatus::Stopped)
    }

    pub fn connected_peers(&self) -> Vec<ConnectedPeerInfo> {
        self.state
            .engine
            .as_ref()
            .map(|engine| engine.peers_rx.borrow().clone())
            .unwrap_or_default()
    }
}

async fn shutdown_engine(mut engine: RunningEngine) {
    let _ = engine.shutdown_tx.send(());
    wait_for_engine_termination(&mut engine.status_rx, ENGINE_STOP_WAIT_TIMEOUT).await;
    abort_bridge_task(engine.event_task).await;
    abort_bridge_task(engine.transfer_task).await;
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
