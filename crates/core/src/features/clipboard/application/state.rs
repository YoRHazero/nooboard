use crate::{ContentKind, contracts::network::TransferStatus};
#[derive(Clone)]
pub(crate) struct SyncState {
    pub current: crate::CurrentClipboard,
    pub operations: std::collections::VecDeque<Operation>,
    pub activities: std::collections::VecDeque<crate::ActivityRecord>,
    pub fault: Option<String>,
    pub application_failures: std::collections::BTreeSet<(crate::MessageId, String)>,
    pub sequence: u64,
    pub effective_revision: u64,
}
#[derive(Clone)]
pub(crate) struct Operation {
    pub incoming_peer: Option<String>,
    pub id: crate::MessageId,
    pub automatic: bool,
    pub bytes: usize,
    pub names: Vec<String>,
    pub kind: Option<ContentKind>,
    pub at_ms: i64,
}

impl Operation {
    pub fn matches(&self, row: &TransferStatus) -> bool {
        self.id == row.id
            && match &self.incoming_peer {
                Some(peer) => row.incoming && peer == &row.peer,
                None => !row.incoming,
            }
    }
}
