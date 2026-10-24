use crate::{HistoryEntry, Result, Settings, contracts::PortFuture};
pub(crate) trait HistoryPort: Send + Sync {
    fn record(&self, text: String, peer: Option<String>, settings: &Settings) -> Result<()>;
    fn get(&self, id: i64) -> PortFuture<'_, HistoryEntry>;
}
