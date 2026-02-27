use std::sync::Arc;

use tokio::sync::{Mutex, broadcast, watch};

use crate::sync_runtime::SyncRuntime;
use crate::{AppError, AppResult};

use super::types::{
    AppEvent, EventStream, EventSubscription, EventSubscriptionItem, SubscriptionCloseReason,
    SubscriptionLifecycle,
};

const EVENT_CHANNEL_CAPACITY: usize = 256;

pub(super) struct SubscriptionHub {
    state: Arc<Mutex<HubState>>,
}

struct HubState {
    next_session_id: u64,
    active: Option<ActiveSession>,
}

struct ActiveSession {
    session_id: u64,
    events_tx: broadcast::Sender<EventSubscriptionItem>,
    cancel_tx: watch::Sender<bool>,
}

enum BridgeOutcome {
    Cancelled,
    Terminated,
}

impl SubscriptionHub {
    pub(super) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HubState {
                next_session_id: 1,
                active: None,
            })),
        }
    }

    pub(super) async fn subscribe(&self) -> AppResult<EventSubscription> {
        let state = self.state.lock().await;
        let active = state.active.as_ref().ok_or(AppError::EngineNotRunning)?;
        Ok(EventSubscription::new(
            active.session_id,
            active.events_tx.subscribe(),
        ))
    }

    pub(super) async fn activate(&self, sync_runtime: Arc<Mutex<SyncRuntime>>) -> AppResult<()> {
        let (sync_rx, transfer_rx, status_rx) = {
            let runtime = sync_runtime.lock().await;
            (
                runtime.subscribe_events()?,
                runtime.subscribe_transfer_updates()?,
                runtime.subscribe_status()?,
            )
        };
        let (events_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let (cancel_tx, cancel_rx) = watch::channel(false);

        let (session_id, previous_session) = {
            let mut state = self.state.lock().await;
            let session_id = state.next_session_id;
            state.next_session_id = state.next_session_id.saturating_add(1);
            let previous_session = state.active.replace(ActiveSession {
                session_id,
                events_tx: events_tx.clone(),
                cancel_tx: cancel_tx.clone(),
            });
            (session_id, previous_session)
        };

        spawn_session_bridge(
            Arc::clone(&self.state),
            session_id,
            sync_rx,
            transfer_rx,
            status_rx,
            events_tx,
            cancel_rx,
        );

        if let Some(previous_session) = previous_session {
            let _ = previous_session
                .events_tx
                .send(EventSubscriptionItem::Lifecycle(
                    SubscriptionLifecycle::Rebinding {
                        from_session_id: previous_session.session_id,
                        to_session_id: session_id,
                    },
                ));
            close_session(
                previous_session,
                SubscriptionCloseReason::Rebinding {
                    next_session_id: session_id,
                },
            )
            .await;
        }

        Ok(())
    }

    pub(super) async fn deactivate(&self, reason: SubscriptionCloseReason) {
        let active = {
            let mut state = self.state.lock().await;
            state.active.take()
        };
        if let Some(active) = active {
            close_session(active, reason).await;
        }
    }
}

async fn close_session(active: ActiveSession, reason: SubscriptionCloseReason) {
    let _ = active.events_tx.send(EventSubscriptionItem::Lifecycle(
        SubscriptionLifecycle::Closed {
            session_id: active.session_id,
            reason,
        },
    ));
    let _ = active.cancel_tx.send(true);
}

#[allow(clippy::too_many_arguments)]
fn spawn_session_bridge(
    state: Arc<Mutex<HubState>>,
    session_id: u64,
    sync_rx: broadcast::Receiver<nooboard_sync::SyncEvent>,
    transfer_rx: broadcast::Receiver<nooboard_sync::TransferUpdate>,
    status_rx: watch::Receiver<nooboard_sync::SyncStatus>,
    events_tx: broadcast::Sender<EventSubscriptionItem>,
    cancel_rx: watch::Receiver<bool>,
) {
    tokio::spawn(async move {
        let outcome = run_session_bridge(
            session_id,
            sync_rx,
            transfer_rx,
            status_rx,
            events_tx,
            cancel_rx,
        )
        .await;
        if matches!(outcome, BridgeOutcome::Cancelled) {
            return;
        }

        let active = {
            let mut state = state.lock().await;
            if state.active.as_ref().map(|active| active.session_id) == Some(session_id) {
                state.active.take()
            } else {
                None
            }
        };
        drop(active);
    });
}

#[allow(clippy::too_many_arguments)]
async fn run_session_bridge(
    session_id: u64,
    mut sync_rx: broadcast::Receiver<nooboard_sync::SyncEvent>,
    mut transfer_rx: broadcast::Receiver<nooboard_sync::TransferUpdate>,
    mut status_rx: watch::Receiver<nooboard_sync::SyncStatus>,
    events_tx: broadcast::Sender<EventSubscriptionItem>,
    mut cancel_rx: watch::Receiver<bool>,
) -> BridgeOutcome {
    let mut status_closed = false;

    loop {
        tokio::select! {
            result = cancel_rx.changed() => {
                match result {
                    Ok(()) if *cancel_rx.borrow() => return BridgeOutcome::Cancelled,
                    Ok(()) => continue,
                    Err(_) => return BridgeOutcome::Cancelled,
                }
            }
            result = status_rx.changed(), if !status_closed => {
                match result {
                    Ok(()) => {
                        if let nooboard_sync::SyncStatus::Error(message) = status_rx.borrow().clone() {
                            emit_fatal(
                                &events_tx,
                                session_id,
                                message,
                            );
                            emit_closed(
                                &events_tx,
                                session_id,
                                SubscriptionCloseReason::Fatal,
                            );
                            return BridgeOutcome::Terminated;
                        }
                    }
                    Err(_) => {
                        status_closed = true;
                    }
                }
            }
            result = sync_rx.recv() => {
                match result {
                    Ok(event) => {
                        if let Err(error) = forward_sync_event(&events_tx, session_id, event) {
                            emit_fatal(&events_tx, session_id, error);
                            emit_closed(
                                &events_tx,
                                session_id,
                                SubscriptionCloseReason::Fatal,
                            );
                            return BridgeOutcome::Terminated;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(dropped)) => {
                        emit_lagged(&events_tx, session_id, EventStream::Sync, dropped);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        emit_fatal(
                            &events_tx,
                            session_id,
                            "sync upstream stream closed".to_string(),
                        );
                        emit_closed(
                            &events_tx,
                            session_id,
                            SubscriptionCloseReason::UpstreamClosed {
                                stream: EventStream::Sync,
                            },
                        );
                        return BridgeOutcome::Terminated;
                    }
                }
            }
            result = transfer_rx.recv() => {
                match result {
                    Ok(update) => {
                        let _ = events_tx.send(EventSubscriptionItem::Event {
                            session_id,
                            event: update.into(),
                        });
                    }
                    Err(broadcast::error::RecvError::Lagged(dropped)) => {
                        emit_lagged(&events_tx, session_id, EventStream::Transfer, dropped);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        emit_fatal(
                            &events_tx,
                            session_id,
                            "transfer upstream stream closed".to_string(),
                        );
                        emit_closed(
                            &events_tx,
                            session_id,
                            SubscriptionCloseReason::UpstreamClosed {
                                stream: EventStream::Transfer,
                            },
                        );
                        return BridgeOutcome::Terminated;
                    }
                }
            }
        }
    }
}

fn forward_sync_event(
    events_tx: &broadcast::Sender<EventSubscriptionItem>,
    session_id: u64,
    event: nooboard_sync::SyncEvent,
) -> Result<(), String> {
    let mapped = AppEvent::try_from(event).map_err(|error| error.to_string())?;
    let _ = events_tx.send(EventSubscriptionItem::Event {
        session_id,
        event: mapped,
    });
    Ok(())
}

fn emit_lagged(
    events_tx: &broadcast::Sender<EventSubscriptionItem>,
    session_id: u64,
    stream: EventStream,
    dropped: u64,
) {
    let _ = events_tx.send(EventSubscriptionItem::Lifecycle(
        SubscriptionLifecycle::Lagged {
            session_id,
            stream,
            dropped,
        },
    ));
}

fn emit_fatal(
    events_tx: &broadcast::Sender<EventSubscriptionItem>,
    session_id: u64,
    error: String,
) {
    let _ = events_tx.send(EventSubscriptionItem::Lifecycle(
        SubscriptionLifecycle::Fatal { session_id, error },
    ));
}

fn emit_closed(
    events_tx: &broadcast::Sender<EventSubscriptionItem>,
    session_id: u64,
    reason: SubscriptionCloseReason,
) {
    let _ = events_tx.send(EventSubscriptionItem::Lifecycle(
        SubscriptionLifecycle::Closed { session_id, reason },
    ));
}

#[cfg(test)]
mod tests {
    use tokio::sync::{broadcast, watch};

    use super::{
        BridgeOutcome, EventStream, EventSubscriptionItem, SubscriptionCloseReason,
        SubscriptionLifecycle, run_session_bridge,
    };

    #[tokio::test(flavor = "current_thread")]
    async fn reports_upstream_close_as_fatal_then_closed() {
        let (events_tx, mut events_rx) = broadcast::channel(8);
        let (sync_tx, sync_rx) = broadcast::channel::<nooboard_sync::SyncEvent>(8);
        drop(sync_tx);
        let (_transfer_tx, transfer_rx) = broadcast::channel::<nooboard_sync::TransferUpdate>(8);
        let (_status_tx, status_rx) = watch::channel(nooboard_sync::SyncStatus::Running);
        let (_cancel_tx, cancel_rx) = watch::channel(false);

        let outcome =
            run_session_bridge(7, sync_rx, transfer_rx, status_rx, events_tx, cancel_rx).await;
        assert!(matches!(outcome, BridgeOutcome::Terminated));
        assert_eq!(
            events_rx.recv().await.expect("fatal"),
            EventSubscriptionItem::Lifecycle(SubscriptionLifecycle::Fatal {
                session_id: 7,
                error: "sync upstream stream closed".to_string(),
            })
        );
        assert_eq!(
            events_rx.recv().await.expect("closed"),
            EventSubscriptionItem::Lifecycle(SubscriptionLifecycle::Closed {
                session_id: 7,
                reason: SubscriptionCloseReason::UpstreamClosed {
                    stream: EventStream::Sync,
                },
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reports_status_error_as_fatal_then_closed() {
        let (events_tx, mut events_rx) = broadcast::channel(8);
        let (sync_tx, sync_rx) = broadcast::channel::<nooboard_sync::SyncEvent>(8);
        let (_transfer_tx, transfer_rx) = broadcast::channel::<nooboard_sync::TransferUpdate>(8);
        let (status_tx, status_rx) = watch::channel(nooboard_sync::SyncStatus::Running);
        let (_cancel_tx, cancel_rx) = watch::channel(false);

        let bridge_task = tokio::spawn(run_session_bridge(
            3,
            sync_rx,
            transfer_rx,
            status_rx,
            events_tx,
            cancel_rx,
        ));
        status_tx
            .send(nooboard_sync::SyncStatus::Error(
                "engine exploded".to_string(),
            ))
            .expect("status send");
        let outcome = bridge_task.await.expect("bridge join");
        assert!(matches!(outcome, BridgeOutcome::Terminated));
        assert_eq!(
            events_rx.recv().await.expect("fatal"),
            EventSubscriptionItem::Lifecycle(SubscriptionLifecycle::Fatal {
                session_id: 3,
                error: "engine exploded".to_string(),
            })
        );
        assert_eq!(
            events_rx.recv().await.expect("closed"),
            EventSubscriptionItem::Lifecycle(SubscriptionLifecycle::Closed {
                session_id: 3,
                reason: SubscriptionCloseReason::Fatal,
            })
        );
        drop(sync_tx);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn emits_lagged_lifecycle_when_upstream_drops_items() {
        let (events_tx, mut events_rx) = broadcast::channel(8);
        let (sync_tx, sync_rx) = broadcast::channel::<nooboard_sync::SyncEvent>(1);
        let (_transfer_tx, transfer_rx) = broadcast::channel::<nooboard_sync::TransferUpdate>(8);
        let (_status_tx, status_rx) = watch::channel(nooboard_sync::SyncStatus::Running);
        let (cancel_tx, cancel_rx) = watch::channel(false);

        sync_tx
            .send(nooboard_sync::SyncEvent::ConnectionError {
                peer_node_id: None,
                addr: None,
                error: "first".to_string(),
            })
            .expect("event send");
        sync_tx
            .send(nooboard_sync::SyncEvent::ConnectionError {
                peer_node_id: None,
                addr: None,
                error: "second".to_string(),
            })
            .expect("event send");

        let bridge_task = tokio::spawn(run_session_bridge(
            11,
            sync_rx,
            transfer_rx,
            status_rx,
            events_tx,
            cancel_rx,
        ));
        assert_eq!(
            events_rx.recv().await.expect("lagged"),
            EventSubscriptionItem::Lifecycle(SubscriptionLifecycle::Lagged {
                session_id: 11,
                stream: EventStream::Sync,
                dropped: 1,
            })
        );

        cancel_tx.send(true).expect("cancel send");
        let outcome = bridge_task.await.expect("bridge join");
        assert!(matches!(outcome, BridgeOutcome::Cancelled));
    }
}
