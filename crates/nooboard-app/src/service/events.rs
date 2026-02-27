use std::sync::Arc;

use tokio::sync::{Mutex, broadcast};

use crate::AppResult;
use crate::sync_runtime::SyncRuntime;

use super::types::{AppEvent, EventStream, SyncEvent};

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
                                forward_sync_event(&events_tx, event);
                            }
                            Err(broadcast::error::RecvError::Lagged(dropped)) => {
                                emit_bridge_lagged(&events_tx, EventStream::Sync, dropped);
                                continue;
                            }
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
                            Err(broadcast::error::RecvError::Lagged(dropped)) => {
                                emit_bridge_lagged(&events_tx, EventStream::Transfer, dropped);
                                continue;
                            }
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

fn forward_sync_event(events_tx: &broadcast::Sender<AppEvent>, event: nooboard_sync::SyncEvent) {
    match AppEvent::try_from(event) {
        Ok(mapped) => {
            let _ = events_tx.send(mapped);
        }
        Err(error) => {
            let _ = events_tx.send(AppEvent::Sync(SyncEvent::BridgeMappingFailed {
                error: error.to_string(),
            }));
        }
    }
}

fn emit_bridge_lagged(events_tx: &broadcast::Sender<AppEvent>, stream: EventStream, dropped: u64) {
    let _ = events_tx.send(AppEvent::Sync(SyncEvent::BridgeLagged {
        stream,
        dropped,
    }));
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast;

    use super::{AppEvent, EventStream, SyncEvent, emit_bridge_lagged, forward_sync_event};

    #[test]
    fn emits_lagged_diagnostic_event() {
        let (events_tx, mut events_rx) = broadcast::channel(8);

        emit_bridge_lagged(&events_tx, EventStream::Transfer, 7);

        assert_eq!(
            events_rx.try_recv().expect("diagnostic event"),
            AppEvent::Sync(SyncEvent::BridgeLagged {
                stream: EventStream::Transfer,
                dropped: 7,
            })
        );
    }

    #[test]
    fn emits_mapping_failure_diagnostic_event() {
        let (events_tx, mut events_rx) = broadcast::channel(8);
        forward_sync_event(
            &events_tx,
            nooboard_sync::SyncEvent::TextReceived {
                event_id: "not-a-uuid".to_string(),
                content: "payload".to_string(),
                device_id: "peer-a".to_string(),
            },
        );

        match events_rx.try_recv().expect("diagnostic event") {
            AppEvent::Sync(SyncEvent::BridgeMappingFailed { error }) => {
                assert!(error.contains("invalid event id"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
