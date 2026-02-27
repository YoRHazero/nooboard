use std::sync::Arc;

use tokio::sync::{Mutex, broadcast};

use crate::AppResult;
use crate::sync_runtime::SyncRuntime;

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
            let mut sync_closed = false;
            let mut transfer_closed = false;

            loop {
                if sync_closed && transfer_closed {
                    break;
                }

                tokio::select! {
                    result = sync_rx.recv(), if !sync_closed => {
                        match result {
                            Ok(event) => {
                                if let Ok(mapped) = AppEvent::try_from(event) {
                                    let _ = events_tx.send(mapped);
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => {
                                sync_closed = true;
                            }
                        }
                    }
                    result = transfer_rx.recv(), if !transfer_closed => {
                        match result {
                            Ok(update) => {
                                let _ = events_tx.send(update.into());
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => {
                                transfer_closed = true;
                            }
                        }
                    }
                }
            }
        });

        *started = true;
        Ok(())
    }
}
