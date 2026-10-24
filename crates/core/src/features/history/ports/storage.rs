use crate::contracts::{PortFuture, storage::*};
pub(crate) trait HistoryStore: Send + Sync {
    fn record(&self, request: RecordHistory) -> PortFuture<'_, RecordOutcome>;
    fn query(&self, request: HistoryQuery) -> PortFuture<'_, HistoryPage>;
    fn get(&self, id: HistoryId) -> PortFuture<'_, Option<HistoryEntry>>;
    fn delete(&self, id: HistoryId) -> PortFuture<'_, bool>;
    fn clear(&self) -> PortFuture<'_, u64>;
    fn prune(&self, retention: Retention) -> PortFuture<'_, u64>;
}
