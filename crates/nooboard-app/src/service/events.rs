use std::sync::Arc;

use tokio::sync::{Mutex, broadcast};

use crate::sync_runtime::SyncRuntime;
use crate::{AppError, AppResult};

use super::types::AppEvent;

const EVENT_CHANNEL_CAPACITY: usize = 256;

pub(super) struct SubscriptionHub {
    events_tx: broadcast::Sender<AppEvent>,
    start_lock: Mutex<bool>,
}

impl SubscriptionHub {
    pub(super) fn new() -> Self {
        let (events_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            events_tx,
            start_lock: Mutex::new(false),
        }
    }

    pub(super) async fn subscribe(
        &self,
        sync_runtime: Arc<Mutex<SyncRuntime>>,
    ) -> AppResult<broadcast::Receiver<AppEvent>> {
        self.ensure_started(sync_runtime).await?;
        Ok(self.events_tx.subscribe())
    }

    async fn ensure_started(&self, sync_runtime: Arc<Mutex<SyncRuntime>>) -> AppResult<()> {
        let mut started = self.start_lock.lock().await;
        if *started {
            return Ok(());
        }

        let (mut sync_rx, mut transfer_rx) = {
            let runtime = sync_runtime.lock().await;
            (
                runtime.subscribe_events(),
                runtime.subscribe_transfer_updates(),
            )
        };

        let events_tx = self.events_tx.clone();
        tokio::spawn(async move {
            loop {
                match sync_rx.recv().await {
                    Ok(event) => {
                        let mapped = AppEvent::try_from(event);
                        match mapped {
                            Ok(event) => {
                                let _ = events_tx.send(event);
                            }
                            Err(AppError::InvalidEventId { .. }) => continue,
                            Err(_) => continue,
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        let events_tx = self.events_tx.clone();
        tokio::spawn(async move {
            loop {
                match transfer_rx.recv().await {
                    Ok(update) => {
                        let _ = events_tx.send(update.into());
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        *started = true;
        Ok(())
    }
}
