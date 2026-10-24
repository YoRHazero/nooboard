use crate::ipc::contracts::*;
pub fn transfers(transfers: Vec<nooboard_core::Transfer>) -> Vec<Transfer> {
    transfers
        .into_iter()
        .map(|t| Transfer {
            id: t.id.into(),
            automatic: t.automatic,
            bytes: t.bytes,
            targets: t
                .targets
                .into_iter()
                .map(|d| Delivery {
                    noob_id: d.noob_id,
                    device_name: d.device_name,
                    state: d.state.into(),
                })
                .collect(),
        })
        .collect()
}
pub fn content_transfers(transfers: Vec<nooboard_core::ContentTransfer>) -> Vec<ContentTransfer> {
    transfers
        .into_iter()
        .map(|t| ContentTransfer {
            key: t.key,
            id: t.id.into(),
            peer: t.peer,
            device_name: t.device_name,
            incoming: t.incoming,
            kind: t.kind.into(),
            names: t.names,
            total_bytes: t.total_bytes,
            completed_bytes: t.completed_bytes,
            stage: t.stage.into(),
            error: t.error.map(Into::into),
            saved_paths: t
                .saved_paths
                .into_iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            at: t.at_ms,
        })
        .collect()
}
