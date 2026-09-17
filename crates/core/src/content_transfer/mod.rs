//! Session-local image/file tasks. Workers own I/O; runtime owns policy and publication.
pub(crate) mod workers;
use crate::link::queue::Outbox;
use nooboard_network::{ContentKind, Message, MessageId, TransferError};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    sync::{Semaphore, mpsc},
    task::JoinHandle,
};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum ContentStage {
    Preparing,
    Queued,
    Waiting,
    Sending,
    Receiving,
    Verifying,
    Saving,
    Applying,
    Cancelling,
    Completed,
    Saved,
    Failed,
    Cancelled,
    Unconfirmed,
}
impl ContentStage {
    pub fn pending(self) -> bool {
        matches!(
            self,
            Self::Preparing
                | Self::Queued
                | Self::Waiting
                | Self::Sending
                | Self::Receiving
                | Self::Verifying
                | Self::Saving
                | Self::Applying
                | Self::Cancelling
        )
    }
    pub fn cancellable(self) -> bool {
        self.pending() && !matches!(self, Self::Saving | Self::Applying | Self::Cancelling)
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct ContentTransfer {
    pub key: String,
    pub id: MessageId,
    pub peer: String,
    pub device_name: String,
    pub incoming: bool,
    pub kind: ContentKind,
    pub names: Vec<String>,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub prepared_bytes: u64,
    pub stage: ContentStage,
    pub error: Option<TransferError>,
    pub saved_paths: Vec<PathBuf>,
    pub at_ms: i64,
}
pub(crate) struct Active {
    pub generation: u64,
    pub epoch: u64,
    pub outbox: Outbox,
    pub cancel: Arc<AtomicBool>,
    pub messages: mpsc::Sender<Message>,
    pub waiting: Option<mpsc::Receiver<Message>>,
    pub task: Option<JoinHandle<()>>,
}
impl Drop for Active {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
pub(crate) struct ContentTransfers {
    pub rows: VecDeque<ContentTransfer>,
    pub active: HashMap<String, Active>,
    pub lanes: HashMap<String, Arc<Semaphore>>,
    pub preparing: HashMap<MessageId, Arc<AtomicBool>>,
    pub events: mpsc::Sender<workers::WorkerEvent>,
    pub incoming: mpsc::Receiver<workers::WorkerEvent>,
    pub apply_lock: Arc<tokio::sync::Mutex<()>>,
    pub image_limit: Arc<Semaphore>,
}
impl Default for ContentTransfers {
    fn default() -> Self {
        let (events, incoming) = mpsc::channel(128);
        Self {
            rows: VecDeque::new(),
            active: HashMap::new(),
            lanes: HashMap::new(),
            preparing: HashMap::new(),
            events,
            incoming,
            apply_lock: Arc::new(tokio::sync::Mutex::new(())),
            image_limit: Arc::new(Semaphore::new(2)),
        }
    }
}
impl ContentTransfers {
    pub fn key(peer: &str, id: &MessageId, incoming: bool) -> String {
        format!(
            "{}:{}:{}:{peer}",
            if incoming { "in" } else { "out" },
            id.session,
            id.sequence
        )
    }
    pub fn snapshot(&self) -> Vec<ContentTransfer> {
        self.rows.iter().cloned().collect()
    }
    pub fn row(&self, key: &str) -> Option<&ContentTransfer> {
        self.rows.iter().find(|t| t.key == key)
    }
    pub fn row_mut(&mut self, key: &str) -> Option<&mut ContentTransfer> {
        self.rows.iter_mut().find(|t| t.key == key)
    }
    pub fn prune(&mut self) {
        let mut completed = 0;
        self.rows.retain(|row| {
            if row.stage.pending() {
                true
            } else {
                completed += 1;
                completed <= 60
            }
        });
    }
    pub fn available(&self) -> bool {
        self.rows.iter().filter(|r| r.stage.pending()).count() < 128
    }
    pub fn stop_unused_preparations(&self) {
        for (id, cancel) in &self.preparing {
            if !self
                .rows
                .iter()
                .any(|row| row.id == *id && row.stage.pending())
            {
                cancel.store(true, Ordering::Release);
            }
        }
    }
    pub fn cancel(&mut self, key: &str) -> bool {
        let Some(row) = self.row(key) else {
            return false;
        };
        if !row.stage.cancellable() {
            return false;
        }
        let id = row.id.clone();
        let wait_for_result =
            !row.incoming && self.active.get(key).is_some_and(|a| a.task.is_some());
        let incoming = row.incoming;
        if let Some(active) = self.active.get(key) {
            active.cancel.store(true, Ordering::Release);
            active.outbox.cancel_bulk(&id);
            let _ = active.outbox.control(if incoming {
                Message::Outcome {
                    id: id.clone(),
                    result: nooboard_network::ContentResult::Cancelled,
                    error: Some(TransferError::Cancelled),
                }
            } else {
                Message::Cancel { id: id.clone() }
            });
        }
        if wait_for_result {
            self.row_mut(key).unwrap().stage = ContentStage::Cancelling;
            return true;
        }
        let row = self.row_mut(key).unwrap();
        row.stage = ContentStage::Cancelled;
        row.error = Some(TransferError::Cancelled);
        self.active.remove(key);
        if !self.rows.iter().any(|r| r.id == id && r.stage.pending())
            && let Some(cancel) = self.preparing.get(&id)
        {
            cancel.store(true, Ordering::Release);
        }
        true
    }
}
impl Drop for ContentTransfers {
    fn drop(&mut self) {
        for cancel in self.preparing.values() {
            cancel.store(true, Ordering::Release);
        }
    }
}
