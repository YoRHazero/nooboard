use crate::error::{CoreError, CoreResult};
use crate::{IncomingTransferDecision, TransferTicket};

use super::super::state::WorkspaceState;
use super::snapshot::finish_with_snapshot_refresh;

pub(crate) async fn send_files(
    state: &mut WorkspaceState,
    request: crate::SendFilesRequest,
) -> CoreResult<Vec<TransferTicket>> {
    let result = state
        .network_runtime()
        .send_files(request)
        .await
        .map_err(Into::into);
    finish_with_snapshot_refresh(state, result).await
}

pub(crate) async fn decide_incoming_transfer(
    state: &mut WorkspaceState,
    decision: IncomingTransferDecision,
) -> CoreResult<()> {
    let result = state
        .network_runtime()
        .decide_incoming_transfer(decision)
        .await
        .map_err(map_transfer_command_error);
    finish_with_snapshot_refresh(state, result).await
}

pub(crate) async fn cancel_transfer(
    state: &mut WorkspaceState,
    ticket: TransferTicket,
) -> CoreResult<()> {
    let result = state
        .network_runtime()
        .cancel_transfer(ticket)
        .await
        .map_err(map_transfer_command_error);
    finish_with_snapshot_refresh(state, result).await
}

fn map_transfer_command_error(error: nooboard_network::NetworkError) -> CoreError {
    match error {
        nooboard_network::NetworkError::TransferNotFound(session_id, raw_id) => {
            CoreError::TransferNotFound {
                ticket: format!("{session_id}:{raw_id}"),
            }
        }
        nooboard_network::NetworkError::TransferNotCancelable(session_id, raw_id) => {
            CoreError::TransferNotCancelable {
                ticket: format!("{session_id}:{raw_id}"),
            }
        }
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use crate::CoreError;

    use super::map_transfer_command_error;

    #[test]
    fn maps_runtime_transfer_not_found_to_core_transfer_not_found() {
        let ticket = crate::TransferTicket {
            session_id: crate::SessionId::new(),
            raw_id: 7,
        };
        let error = map_transfer_command_error(nooboard_network::NetworkError::TransferNotFound(
            ticket.session_id,
            ticket.raw_id,
        ));

        assert!(matches!(
            error,
            CoreError::TransferNotFound { ticket: value }
                if value == format!("{}:{}", ticket.session_id, ticket.raw_id)
        ));
    }

    #[test]
    fn maps_runtime_transfer_not_cancelable_to_core_transfer_not_cancelable() {
        let ticket = crate::TransferTicket {
            session_id: crate::SessionId::new(),
            raw_id: 11,
        };
        let error = map_transfer_command_error(
            nooboard_network::NetworkError::TransferNotCancelable(ticket.session_id, ticket.raw_id),
        );

        assert!(matches!(
            error,
            CoreError::TransferNotCancelable { ticket: value }
                if value == format!("{}:{}", ticket.session_id, ticket.raw_id)
        ));
    }
}
