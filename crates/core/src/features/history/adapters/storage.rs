use crate::{
    contracts::{PortFuture, storage::*},
    features::history::ports::storage::HistoryStore,
};
use std::sync::Arc;
struct HistoryAdapter(nooboard_storage::Storage);
pub(crate) fn port(storage: nooboard_storage::Storage) -> Arc<dyn HistoryStore> {
    Arc::new(HistoryAdapter(storage))
}
impl HistoryStore for HistoryAdapter {
    fn record(&self, request: RecordHistory) -> PortFuture<'_, RecordOutcome> {
        Box::pin(async move { self.0.history().record(request).await.map_err(Into::into) })
    }
    fn query(&self, request: HistoryQuery) -> PortFuture<'_, HistoryPage> {
        Box::pin(async move { self.0.history().query(request).await.map_err(Into::into) })
    }
    fn get(&self, id: HistoryId) -> PortFuture<'_, Option<HistoryEntry>> {
        Box::pin(async move { self.0.history().get(id).await.map_err(Into::into) })
    }
    fn delete(&self, id: HistoryId) -> PortFuture<'_, bool> {
        Box::pin(async move { self.0.history().delete(id).await.map_err(Into::into) })
    }
    fn clear(&self) -> PortFuture<'_, u64> {
        Box::pin(async move { self.0.history().clear().await.map_err(Into::into) })
    }
    fn prune(&self, retention: Retention) -> PortFuture<'_, u64> {
        Box::pin(async move { self.0.history().prune(retention).await.map_err(Into::into) })
    }
}

impl From<nooboard_storage::HistoryEntry> for crate::HistoryEntry {
    fn from(row: nooboard_storage::HistoryEntry) -> Self {
        Self {
            id: row.id.value(),
            text: row.entry.text,
            source: match row.entry.source {
                nooboard_storage::HistorySource::Local => "local".into(),
                nooboard_storage::HistorySource::Remote(peer) => peer,
            },
            copied_at_ms: row.entry.copied_at_ms,
        }
    }
}
