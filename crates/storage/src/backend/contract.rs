use crate::{
    HistoryEntry, HistoryId, HistoryPage, HistoryQuery, RecordHistory, RecordOutcome, Result,
    Retention, SettingsChanges,
};
use std::{future::Future, pin::Pin};

pub(crate) type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// Database-independent operations on validated inputs.
///
/// Record (including exact-text deduplication and retention), prune, clear and
/// settings batches each commit atomically. Successful writes are visible to
/// subsequent reads. No operation silently retries an uncertain commit.
/// Queries are read-only; source/search filtering precedes pagination, ordered
/// by descending copy time and a deterministic backend tie-breaker.
/// Implementations must enforce these guarantees across connections as well as
/// within this service. A future PostgreSQL adapter owns its isolation/locking.
pub(crate) trait Backend: Send + Sync {
    fn record_history(&self, record: RecordHistory) -> BackendFuture<'_, RecordOutcome>;
    fn query_history(&self, query: HistoryQuery) -> BackendFuture<'_, HistoryPage>;
    fn get_history(&self, id: HistoryId) -> BackendFuture<'_, Option<HistoryEntry>>;
    fn delete_history(&self, id: HistoryId) -> BackendFuture<'_, bool>;
    fn prune_history(&self, retention: Retention) -> BackendFuture<'_, u64>;
    fn clear_history(&self) -> BackendFuture<'_, u64>;
    /// One snapshot; output positions correspond to the supplied keys.
    fn read_settings(&self, keys: Vec<String>) -> BackendFuture<'_, Vec<Option<String>>>;
    fn apply_settings(&self, changes: SettingsChanges) -> BackendFuture<'_, ()>;
    fn close(&self) -> BackendFuture<'_, ()>;
}
