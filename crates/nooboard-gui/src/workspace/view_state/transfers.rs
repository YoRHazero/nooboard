use nooboard_core::WorkspaceSnapshot;

#[derive(Clone)]
pub struct TransfersPageViewState {
    pub incoming: Vec<String>,
    pub active: Vec<String>,
    pub completed: Vec<String>,
}

pub(super) fn build_transfers_page_view_state(
    snapshot: &WorkspaceSnapshot,
) -> TransfersPageViewState {
    TransfersPageViewState {
        incoming: snapshot
            .network
            .transfers
            .incoming_pending
            .iter()
            .map(|transfer| {
                format!(
                    "{} · {} bytes · {}",
                    transfer.file_name, transfer.file_size, transfer.peer_device_id
                )
            })
            .collect(),
        active: snapshot
            .network
            .transfers
            .active
            .iter()
            .map(|transfer| {
                format!(
                    "{} · {} / {} bytes · {:?}",
                    transfer.file_name,
                    transfer.transferred_bytes,
                    transfer.file_size,
                    transfer.state
                )
            })
            .collect(),
        completed: snapshot
            .network
            .transfers
            .recent_completed
            .iter()
            .map(|transfer| {
                format!(
                    "{} · {:?} · {}",
                    transfer.file_name, transfer.outcome, transfer.peer_device_id
                )
            })
            .collect(),
    }
}
