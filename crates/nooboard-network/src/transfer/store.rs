use std::collections::{BTreeMap, VecDeque};

use crate::{
    ActiveTransferInfo, CompletedTransferInfo, IncomingTransferOffer, SessionId, TransferTicket,
    TransfersSnapshot,
};

const RECENT_COMPLETED_LIMIT: usize = 256;

#[derive(Debug, Default)]
pub(crate) struct TransferStore {
    incoming_pending: BTreeMap<TransferTicket, IncomingTransferOffer>,
    active: BTreeMap<TransferTicket, ActiveTransferInfo>,
    recent_completed: VecDeque<CompletedTransferInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransferCommandState {
    PendingDecision,
    Active,
    Completed,
    Missing,
}

impl TransferStore {
    pub(crate) fn snapshot(&self) -> TransfersSnapshot {
        let mut incoming_pending: Vec<_> = self.incoming_pending.values().cloned().collect();
        incoming_pending.sort_by_key(|offer| (offer.offered_at_ms, offer.ticket.raw_id));

        let mut active: Vec<_> = self.active.values().cloned().collect();
        active.sort_by_key(|transfer| (transfer.updated_at_ms, transfer.ticket.raw_id));

        let mut recent_completed: Vec<_> = self.recent_completed.iter().cloned().collect();
        recent_completed.sort_by(|left, right| {
            right
                .finished_at_ms
                .cmp(&left.finished_at_ms)
                .then_with(|| left.ticket.raw_id.cmp(&right.ticket.raw_id))
        });

        TransfersSnapshot {
            incoming_pending,
            active,
            recent_completed,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.incoming_pending.clear();
        self.active.clear();
        self.recent_completed.clear();
    }

    pub(crate) fn classify(&self, ticket: TransferTicket) -> TransferCommandState {
        if self.incoming_pending.contains_key(&ticket) {
            return TransferCommandState::PendingDecision;
        }
        if self.active.contains_key(&ticket) {
            return TransferCommandState::Active;
        }
        if self
            .recent_completed
            .iter()
            .any(|transfer| transfer.ticket == ticket)
        {
            return TransferCommandState::Completed;
        }
        TransferCommandState::Missing
    }

    pub(crate) fn apply_queued_upload(&mut self, transfer: ActiveTransferInfo) {
        self.incoming_pending.remove(&transfer.ticket);
        self.active.insert(transfer.ticket, transfer);
    }

    pub(crate) fn apply_offer(&mut self, offer: IncomingTransferOffer) {
        self.incoming_pending.insert(offer.ticket, offer);
    }

    pub(crate) fn apply_update(&mut self, mut update: ActiveTransferInfo) {
        if let Some(existing) = self.active.get(&update.ticket) {
            preserve_transfer_metadata(&mut update.file_name, &existing.file_name);
            preserve_transfer_file_size(&mut update.file_size, existing.file_size);
        } else if let Some(offer) = self.incoming_pending.get(&update.ticket) {
            preserve_transfer_metadata(&mut update.file_name, &offer.file_name);
            preserve_transfer_file_size(&mut update.file_size, offer.file_size);
        }

        self.incoming_pending.remove(&update.ticket);
        self.active.insert(update.ticket, update);
    }

    pub(crate) fn apply_completed(&mut self, mut completed: CompletedTransferInfo) {
        if let Some(existing) = self.active.remove(&completed.ticket) {
            preserve_transfer_metadata(&mut completed.file_name, &existing.file_name);
            preserve_transfer_file_size(&mut completed.file_size, existing.file_size);
        } else if let Some(offer) = self.incoming_pending.remove(&completed.ticket) {
            preserve_transfer_metadata(&mut completed.file_name, &offer.file_name);
            preserve_transfer_file_size(&mut completed.file_size, offer.file_size);
        }

        self.recent_completed.push_front(completed);
        while self.recent_completed.len() > RECENT_COMPLETED_LIMIT {
            let _ = self.recent_completed.pop_back();
        }
    }

    pub(crate) fn remove_session(&mut self, session_id: SessionId) {
        self.incoming_pending
            .retain(|ticket, _| ticket.session_id != session_id);
        self.active
            .retain(|ticket, _| ticket.session_id != session_id);
        self.recent_completed
            .retain(|transfer| transfer.session_id != session_id);
    }
}

fn preserve_transfer_metadata(target: &mut String, source: &str) {
    if target.is_empty() && !source.is_empty() {
        *target = source.to_string();
    }
}

fn preserve_transfer_file_size(target: &mut u64, source: u64) {
    if *target == 0 && source != 0 {
        *target = source;
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ActiveTransferInfo, ActiveTransferState, CompletedTransferInfo, IncomingTransferOffer,
        SessionId, TransferDirection, TransferOutcome, TransferTicket,
    };

    use super::{TransferCommandState, TransferStore};

    fn ticket() -> TransferTicket {
        TransferTicket {
            session_id: SessionId::new(),
            raw_id: 1,
        }
    }

    #[test]
    fn update_inherits_offer_metadata() {
        let mut store = TransferStore::default();
        let ticket = ticket();
        store.apply_offer(IncomingTransferOffer {
            ticket,
            session_id: ticket.session_id,
            peer_noob_id: "peer".to_string(),
            peer_device_id: "device".to_string(),
            file_name: "demo.txt".to_string(),
            file_size: 42,
            total_chunks: 1,
            offered_at_ms: 1,
        });

        store.apply_update(ActiveTransferInfo {
            ticket,
            session_id: ticket.session_id,
            peer_noob_id: "peer".to_string(),
            peer_device_id: "device".to_string(),
            file_name: String::new(),
            file_size: 0,
            transferred_bytes: 10,
            direction: TransferDirection::Download,
            state: ActiveTransferState::InProgress,
            updated_at_ms: 2,
        });

        let snapshot = store.snapshot();
        assert!(snapshot.incoming_pending.is_empty());
        assert_eq!(snapshot.active[0].file_name, "demo.txt");
        assert_eq!(snapshot.active[0].file_size, 42);
    }

    #[test]
    fn completed_inherits_existing_metadata() {
        let mut store = TransferStore::default();
        let ticket = ticket();
        store.apply_update(ActiveTransferInfo {
            ticket,
            session_id: ticket.session_id,
            peer_noob_id: "peer".to_string(),
            peer_device_id: "device".to_string(),
            file_name: "demo.txt".to_string(),
            file_size: 42,
            transferred_bytes: 10,
            direction: TransferDirection::Upload,
            state: ActiveTransferState::InProgress,
            updated_at_ms: 2,
        });
        store.apply_completed(CompletedTransferInfo {
            ticket,
            session_id: ticket.session_id,
            peer_noob_id: "peer".to_string(),
            peer_device_id: "device".to_string(),
            file_name: String::new(),
            file_size: 0,
            direction: TransferDirection::Upload,
            outcome: TransferOutcome::Succeeded,
            saved_path: None,
            message: None,
            finished_at_ms: 3,
        });

        let snapshot = store.snapshot();
        assert!(snapshot.active.is_empty());
        assert_eq!(snapshot.recent_completed[0].file_name, "demo.txt");
        assert_eq!(snapshot.recent_completed[0].file_size, 42);
    }

    #[test]
    fn classifies_ticket_across_pending_active_and_completed() {
        let mut store = TransferStore::default();
        let ticket = ticket();

        assert_eq!(store.classify(ticket), TransferCommandState::Missing);

        store.apply_offer(IncomingTransferOffer {
            ticket,
            session_id: ticket.session_id,
            peer_noob_id: "peer".to_string(),
            peer_device_id: "device".to_string(),
            file_name: "demo.txt".to_string(),
            file_size: 42,
            total_chunks: 1,
            offered_at_ms: 1,
        });
        assert_eq!(
            store.classify(ticket),
            TransferCommandState::PendingDecision
        );

        store.apply_queued_upload(ActiveTransferInfo {
            ticket,
            session_id: ticket.session_id,
            peer_noob_id: "peer".to_string(),
            peer_device_id: "device".to_string(),
            file_name: "demo.txt".to_string(),
            file_size: 42,
            transferred_bytes: 0,
            direction: TransferDirection::Upload,
            state: ActiveTransferState::Queued,
            updated_at_ms: 2,
        });
        assert_eq!(store.classify(ticket), TransferCommandState::Active);

        store.apply_completed(CompletedTransferInfo {
            ticket,
            session_id: ticket.session_id,
            peer_noob_id: "peer".to_string(),
            peer_device_id: "device".to_string(),
            file_name: String::new(),
            file_size: 0,
            direction: TransferDirection::Upload,
            outcome: TransferOutcome::Cancelled,
            saved_path: None,
            message: None,
            finished_at_ms: 3,
        });
        assert_eq!(store.classify(ticket), TransferCommandState::Completed);
    }
}
