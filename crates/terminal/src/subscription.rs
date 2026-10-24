use crate::{TerminalEvent, TerminalServiceOptions, TerminalSnapshot, TerminalSubscriptionError};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{mpsc, watch};

pub(crate) struct Shared {
    pub(crate) view: Mutex<View>,
    changed: watch::Sender<Option<TerminalSnapshot>>,
}
pub(crate) struct View {
    pub(crate) snapshot: TerminalSnapshot,
    pub(crate) options: TerminalServiceOptions,
    subscribers: Vec<Subscriber>,
}
struct Subscriber {
    tx: mpsc::Sender<TerminalEvent>,
    lagged: Arc<AtomicBool>,
}
pub struct TerminalSubscription {
    pub initial_snapshot: TerminalSnapshot,
    pub snapshots: TerminalSnapshotStream,
    pub events: TerminalEventStream,
}
pub struct TerminalSnapshotStream {
    rx: watch::Receiver<Option<TerminalSnapshot>>,
    done: bool,
}
pub struct TerminalEventStream {
    rx: mpsc::Receiver<TerminalEvent>,
    lagged: Arc<AtomicBool>,
    done: bool,
}
impl Shared {
    pub(crate) fn new(options: TerminalServiceOptions) -> Arc<Self> {
        let snapshot = TerminalSnapshot::default();
        let (changed, _) = watch::channel(Some(snapshot.clone()));
        Arc::new(Self {
            view: Mutex::new(View {
                snapshot,
                options,
                subscribers: vec![],
            }),
            changed,
        })
    }
    pub(crate) fn subscribe(&self) -> TerminalSubscription {
        let mut view = self.view.lock().unwrap();
        let (tx, rx) = mpsc::channel(view.options.event_buffer_capacity);
        let lagged = Arc::new(AtomicBool::new(false));
        if self.changed.borrow().is_some() {
            view.subscribers.push(Subscriber {
                tx,
                lagged: lagged.clone(),
            });
        }
        TerminalSubscription {
            initial_snapshot: view.snapshot.clone(),
            snapshots: TerminalSnapshotStream {
                rx: self.changed.subscribe(),
                done: false,
            },
            events: TerminalEventStream {
                rx,
                lagged,
                done: false,
            },
        }
    }
    pub(crate) fn publish(&self, snapshot: TerminalSnapshot, events: Vec<TerminalEvent>) {
        let mut view = self.view.lock().unwrap();
        view.snapshot = snapshot.clone();
        self.changed.send_replace(Some(snapshot));
        for event in events {
            view.subscribers
                .retain(|subscriber| match subscriber.tx.try_send(event.clone()) {
                    Ok(()) => true,
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        subscriber.lagged.store(true, Ordering::Release);
                        false
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => false,
                });
        }
    }
    pub(crate) fn close(&self) {
        let mut view = self.view.lock().unwrap();
        view.subscribers.clear();
        self.changed.send_replace(None);
    }
}
impl TerminalSnapshotStream {
    pub async fn next(&mut self) -> Option<TerminalSnapshot> {
        if self.done {
            return None;
        }
        if self.rx.borrow().is_none() || self.rx.changed().await.is_err() {
            self.done = true;
            return None;
        }
        let next = self.rx.borrow_and_update().clone();
        if next.is_none() {
            self.done = true;
        }
        next
    }
}
impl TerminalEventStream {
    pub async fn next(
        &mut self,
    ) -> std::result::Result<Option<TerminalEvent>, TerminalSubscriptionError> {
        if self.done {
            return Ok(None);
        }
        let next = self.rx.recv().await;
        if self.lagged.load(Ordering::Acquire) {
            self.done = true;
            self.rx.close();
            return Err(TerminalSubscriptionError::Lagged);
        }
        if next.is_none() {
            self.done = true;
        }
        Ok(next)
    }
}
