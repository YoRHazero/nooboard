use crate::error::{CoreError, CoreResult};
use crate::{IncomingTransferDecision, TransferTicket};

use super::super::state::WorkspaceState;
use super::network::refresh_snapshot_from_runtime;

pub(crate) async fn send_files(
    state: &mut WorkspaceState,
    request: crate::SendFilesRequest,
) -> CoreResult<Vec<TransferTicket>> {
    let result = state
        .network_runtime()
        .send_files(request)
        .await
        .map_err(Into::into);
    finish_after_refresh(result, refresh_snapshot_from_runtime(state).await)
}

pub(crate) async fn decide_incoming_transfer(
    state: &mut WorkspaceState,
    decision: IncomingTransferDecision,
) -> CoreResult<()> {
    match classify_ticket(&state.current_snapshot().network, decision.ticket) {
        TransferTicketState::PendingDecision => {}
        TransferTicketState::Active | TransferTicketState::Completed => {
            return Err(CoreError::TransferNotCancelable {
                ticket: format!("{}:{}", decision.ticket.session_id, decision.ticket.raw_id),
            });
        }
        TransferTicketState::Missing => {
            return Err(CoreError::TransferNotFound {
                ticket: format!("{}:{}", decision.ticket.session_id, decision.ticket.raw_id),
            });
        }
    }

    let result = state
        .network_runtime()
        .decide_incoming_transfer(decision)
        .await
        .map_err(Into::into);
    finish_after_refresh(result, refresh_snapshot_from_runtime(state).await)
}

pub(crate) async fn cancel_transfer(
    state: &mut WorkspaceState,
    ticket: TransferTicket,
) -> CoreResult<()> {
    match classify_ticket(&state.current_snapshot().network, ticket) {
        TransferTicketState::Active => {}
        TransferTicketState::PendingDecision | TransferTicketState::Completed => {
            return Err(CoreError::TransferNotCancelable {
                ticket: format!("{}:{}", ticket.session_id, ticket.raw_id),
            });
        }
        TransferTicketState::Missing => {
            return Err(CoreError::TransferNotFound {
                ticket: format!("{}:{}", ticket.session_id, ticket.raw_id),
            });
        }
    }

    let result = state
        .network_runtime()
        .cancel_transfer(ticket)
        .await
        .map_err(Into::into);
    finish_after_refresh(result, refresh_snapshot_from_runtime(state).await)
}

fn finish_after_refresh<T>(result: CoreResult<T>, refresh: CoreResult<()>) -> CoreResult<T> {
    match result {
        Ok(value) => {
            refresh?;
            Ok(value)
        }
        Err(error) => Err(error),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransferTicketState {
    PendingDecision,
    Active,
    Completed,
    Missing,
}

fn classify_ticket(
    snapshot: &nooboard_network::NetworkSnapshot,
    ticket: TransferTicket,
) -> TransferTicketState {
    if snapshot
        .transfers
        .incoming_pending
        .iter()
        .any(|offer| offer.ticket == ticket)
    {
        return TransferTicketState::PendingDecision;
    }
    if snapshot
        .transfers
        .active
        .iter()
        .any(|transfer| transfer.ticket == ticket)
    {
        return TransferTicketState::Active;
    }
    if snapshot
        .transfers
        .recent_completed
        .iter()
        .any(|transfer| transfer.ticket == ticket)
    {
        return TransferTicketState::Completed;
    }
    TransferTicketState::Missing
}

#[cfg(test)]
mod tests {
    use crate::{SessionId, TransferTicket};

    use super::{TransferTicketState, classify_ticket};

    #[test]
    fn classifies_transfer_ticket_from_network_snapshot() {
        let ticket = TransferTicket {
            session_id: SessionId::new(),
            raw_id: 7,
        };

        let mut snapshot = nooboard_network::NetworkSnapshot {
            status: nooboard_network::NetworkStatus::Stopped,
            lan_enabled: false,
            lan_peers: Vec::new(),
            direct_seeds: Vec::new(),
            pending_direct_requests: Vec::new(),
            sessions: Vec::new(),
            transfers: Default::default(),
        };

        snapshot
            .transfers
            .incoming_pending
            .push(nooboard_network::IncomingTransferOffer {
                ticket,
                session_id: ticket.session_id,
                peer_noob_id: "peer".to_string(),
                peer_device_id: "peer-device".to_string(),
                file_name: "file.txt".to_string(),
                file_size: 1,
                total_chunks: 1,
                offered_at_ms: 0,
            });
        assert_eq!(
            classify_ticket(&snapshot, ticket),
            TransferTicketState::PendingDecision
        );

        snapshot.transfers.incoming_pending.clear();
        snapshot
            .transfers
            .active
            .push(nooboard_network::ActiveTransferInfo {
                ticket,
                session_id: ticket.session_id,
                peer_noob_id: "peer".to_string(),
                peer_device_id: "peer-device".to_string(),
                file_name: "file.txt".to_string(),
                file_size: 1,
                transferred_bytes: 0,
                direction: nooboard_network::TransferDirection::Download,
                state: nooboard_network::ActiveTransferState::Queued,
                updated_at_ms: 0,
            });
        assert_eq!(
            classify_ticket(&snapshot, ticket),
            TransferTicketState::Active
        );
    }
}
