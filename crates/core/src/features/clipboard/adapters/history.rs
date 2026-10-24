use crate::{
    HistoryEntry, Result, Settings,
    contracts::PortFuture,
    features::{clipboard::ports::history::HistoryPort, history::History},
};
impl HistoryPort for History {
    fn record(&self, text: String, peer: Option<String>, settings: &Settings) -> Result<()> {
        self.record(text, peer, settings)
    }
    fn get(&self, id: i64) -> PortFuture<'_, HistoryEntry> {
        Box::pin(self.get(id))
    }
}
