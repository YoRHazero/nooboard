#[derive(Clone, Default)]
pub(crate) struct HistoryState {
    pub revision: u64,
    pub error: Option<String>,
}
