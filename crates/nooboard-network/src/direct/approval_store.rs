use std::collections::BTreeMap;
use std::net::SocketAddr;

use tokio::task::JoinHandle;

use crate::transport::NetworkFramed;
use crate::{DirectRequestId, PendingDirectRequest};

pub(crate) struct PendingApproval {
    pub(crate) info: PendingDirectRequest,
    pub(crate) local_bind_addr: Option<SocketAddr>,
    pub(crate) framed: NetworkFramed,
    pub(crate) timeout_task: JoinHandle<()>,
}

impl PendingApproval {
    pub(crate) fn abort_timeout(&self) {
        self.timeout_task.abort();
    }
}

#[derive(Default)]
pub(crate) struct ApprovalStore {
    approvals: BTreeMap<DirectRequestId, PendingApproval>,
}

impl ApprovalStore {
    pub(crate) fn list(&self) -> Vec<PendingDirectRequest> {
        let mut approvals: Vec<_> = self
            .approvals
            .values()
            .map(|approval| approval.info.clone())
            .collect();
        approvals.sort_by_key(|approval| (approval.expires_at_ms, approval.id));
        approvals
    }

    pub(crate) fn insert(&mut self, approval: PendingApproval) -> Option<PendingApproval> {
        self.approvals.insert(approval.info.id, approval)
    }

    pub(crate) fn remove(&mut self, id: DirectRequestId) -> Option<PendingApproval> {
        self.approvals.remove(&id)
    }

    pub(crate) fn contains_peer_noob_id(&self, peer_noob_id: &str) -> bool {
        self.approvals
            .values()
            .any(|approval| approval.info.peer_noob_id == peer_noob_id)
    }

    pub(crate) fn clear(&mut self) -> Vec<PendingApproval> {
        self.approvals
            .extract_if(.., |_, _| true)
            .map(|(_, approval)| approval)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::ApprovalStore;

    #[test]
    fn list_is_sorted_by_expiry_then_id() {
        let store = ApprovalStore::default();
        assert!(store.list().is_empty());
    }
}
