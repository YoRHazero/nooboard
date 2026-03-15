use std::path::PathBuf;

use nooboard_core::{TransferTicket, WorkspaceSnapshot};

use super::WorkspaceSessionTargetViewState;

#[derive(Clone)]
pub struct TransfersPageViewState {
    pub available_targets: Vec<WorkspaceSessionTargetViewState>,
    pub incoming: Vec<IncomingTransferViewState>,
    pub active: Vec<ActiveTransferViewState>,
    pub completed: Vec<CompletedTransferViewState>,
}

#[derive(Clone)]
pub struct IncomingTransferViewState {
    pub ticket: TransferTicket,
    pub peer_device_id: String,
    pub peer_noob_id: String,
    pub file_name: String,
    pub size_label: String,
}

#[derive(Clone)]
pub struct ActiveTransferViewState {
    pub ticket: TransferTicket,
    pub peer_device_id: String,
    pub peer_noob_id: String,
    pub file_name: String,
    pub progress_label: String,
    pub direction_label: String,
    pub state_label: String,
}

#[derive(Clone)]
pub struct CompletedTransferViewState {
    pub ticket: TransferTicket,
    pub peer_device_id: String,
    pub peer_noob_id: String,
    pub file_name: String,
    pub direction_label: String,
    pub outcome_label: String,
    pub saved_path: Option<PathBuf>,
    pub message: Option<String>,
}

pub(super) fn build_transfers_page_view_state(
    snapshot: &WorkspaceSnapshot,
) -> TransfersPageViewState {
    TransfersPageViewState {
        available_targets: snapshot
            .network
            .sessions
            .iter()
            .map(|session| WorkspaceSessionTargetViewState {
                id: session.id,
                device_id: session.peer_device_id.clone(),
                mode_label: format!("{:?}", session.mode),
                remote_addr_label: session.remote_addr.to_string(),
            })
            .collect(),
        incoming: snapshot
            .network
            .transfers
            .incoming_pending
            .iter()
            .map(|transfer| IncomingTransferViewState {
                ticket: transfer.ticket,
                peer_device_id: transfer.peer_device_id.clone(),
                peer_noob_id: transfer.peer_noob_id.clone(),
                file_name: transfer.file_name.clone(),
                size_label: bytes_to_label(transfer.file_size),
            })
            .collect(),
        active: snapshot
            .network
            .transfers
            .active
            .iter()
            .map(|transfer| ActiveTransferViewState {
                ticket: transfer.ticket,
                peer_device_id: transfer.peer_device_id.clone(),
                peer_noob_id: transfer.peer_noob_id.clone(),
                file_name: transfer.file_name.clone(),
                progress_label: format!(
                    "{} / {}",
                    bytes_to_label(transfer.transferred_bytes),
                    bytes_to_label(transfer.file_size)
                ),
                direction_label: format!("{:?}", transfer.direction),
                state_label: format!("{:?}", transfer.state),
            })
            .collect(),
        completed: snapshot
            .network
            .transfers
            .recent_completed
            .iter()
            .map(|transfer| CompletedTransferViewState {
                ticket: transfer.ticket,
                peer_device_id: transfer.peer_device_id.clone(),
                peer_noob_id: transfer.peer_noob_id.clone(),
                file_name: transfer.file_name.clone(),
                direction_label: format!("{:?}", transfer.direction),
                outcome_label: format!("{:?}", transfer.outcome),
                saved_path: transfer.saved_path.clone(),
                message: transfer.message.clone(),
            })
            .collect(),
    }
}

fn bytes_to_label(value: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let value = value as f64;
    if value >= GB {
        format!("{:.1} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{} B", value as u64)
    }
}
