use crate::{
    Error, HistoryEntry, HistoryId, HistoryPage, HistoryQuery, RecordHistory, RecordOutcome,
    Result, Retention, SettingsChanges, backend::contract::Backend,
};
use tokio::sync::oneshot;

pub(crate) type Reply<T> = oneshot::Sender<Result<T>>;
pub(crate) enum Request {
    Record {
        input: RecordHistory,
        reply: Reply<RecordOutcome>,
    },
    Query {
        input: HistoryQuery,
        reply: Reply<HistoryPage>,
    },
    Get {
        id: HistoryId,
        reply: Reply<Option<HistoryEntry>>,
    },
    Delete {
        id: HistoryId,
        reply: Reply<bool>,
    },
    Prune {
        retention: Retention,
        reply: Reply<u64>,
    },
    Clear {
        reply: Reply<u64>,
    },
    ReadSettings {
        keys: Vec<String>,
        reply: Reply<Vec<Option<String>>>,
    },
    ApplySettings {
        changes: SettingsChanges,
        reply: Reply<()>,
    },
}
impl Request {
    pub async fn execute(self, backend: &dyn Backend) {
        // Once an operation begins, always await it, even if its caller cancels.
        // Dropping a future is not a transaction rollback.
        match self {
            Self::Record { input, reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(backend.record_history(input).await);
                }
            }
            Self::Query { input, reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(backend.query_history(input).await);
                }
            }
            Self::Get { id, reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(backend.get_history(id).await);
                }
            }
            Self::Delete { id, reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(backend.delete_history(id).await);
                }
            }
            Self::Prune { retention, reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(backend.prune_history(retention).await);
                }
            }
            Self::Clear { reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(backend.clear_history().await);
                }
            }
            Self::ReadSettings { keys, reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(backend.read_settings(keys).await);
                }
            }
            Self::ApplySettings { changes, reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(backend.apply_settings(changes).await);
                }
            }
        }
    }
    pub fn reject(self) {
        match self {
            Self::Record { reply, .. } => {
                let _ = reply.send(Err(Error::stopped()));
            }
            Self::Query { reply, .. } => {
                let _ = reply.send(Err(Error::stopped()));
            }
            Self::Get { reply, .. } => {
                let _ = reply.send(Err(Error::stopped()));
            }
            Self::Delete { reply, .. } => {
                let _ = reply.send(Err(Error::stopped()));
            }
            Self::Prune { reply, .. } | Self::Clear { reply } => {
                let _ = reply.send(Err(Error::stopped()));
            }
            Self::ReadSettings { reply, .. } => {
                let _ = reply.send(Err(Error::stopped()));
            }
            Self::ApplySettings { reply, .. } => {
                let _ = reply.send(Err(Error::stopped()));
            }
        }
    }
}
