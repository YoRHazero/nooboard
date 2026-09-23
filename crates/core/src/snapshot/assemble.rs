use crate::sync::model::{content_stage, transfer_key};
use crate::{
    configuration::model::Configuration, devices::model::DeviceState, history::model::HistoryState,
    sync::model::SyncState, *,
};
use nooboard_network::{NetworkStatus, TransferStage};
fn delivery(stage: TransferStage) -> DeliveryState {
    match stage {
        TransferStage::Preparing | TransferStage::Queued | TransferStage::WaitingForAcceptance => {
            DeliveryState::Queued
        }
        TransferStage::Sending | TransferStage::Receiving => DeliveryState::Sending,
        TransferStage::WaitingForApplication
        | TransferStage::AwaitingReceipt
        | TransferStage::Cancelling => DeliveryState::AwaitingReceipt,
        TransferStage::Applied => DeliveryState::Applied,
        TransferStage::Saved | TransferStage::Unconfirmed => DeliveryState::Unconfirmed,
        TransferStage::Rejected => DeliveryState::Rejected,
        TransferStage::Cancelled => DeliveryState::Cancelled,
        TransferStage::Superseded => DeliveryState::Superseded,
        TransferStage::Failed => DeliveryState::Offline,
    }
}
pub(crate) struct Inputs<'a> {
    pub configuration: &'a Configuration,
    pub devices: &'a DeviceState,
    pub sync: &'a SyncState,
    pub history: &'a HistoryState,
    pub network: &'a NetworkStatus,
    pub startup: &'a Settings,
}
pub(crate) fn assemble(
    input: Inputs<'_>,
    session: &str,
    revision: u64,
    state: AppState,
) -> AppSnapshot {
    let Inputs {
        configuration: c,
        devices: d,
        sync: s,
        history: h,
        network: n,
        startup,
    } = input;
    let name = |id: &str| {
        c.peers
            .get(id)
            .map_or_else(|| id.to_owned(), |p| p.trusted.identity.device_name.clone())
    };
    let transfers = s
        .operations
        .iter()
        .filter(|op| op.kind.is_none() && op.incoming_peer.is_none())
        .filter_map(|op| {
            let targets = n
                .transfers
                .iter()
                .filter(|r| op.matches(r))
                .map(|r| Delivery {
                    noob_id: r.peer.clone(),
                    device_name: name(&r.peer),
                    state: delivery(r.stage),
                })
                .collect::<Vec<_>>();
            (!targets.is_empty()).then(|| Transfer {
                id: op.id.clone(),
                automatic: op.automatic,
                bytes: op.bytes,
                targets,
            })
        })
        .collect::<Vec<_>>();
    let content_transfers = n
        .transfers
        .iter()
        .filter_map(|row| {
            let op = s.operations.iter().find(|op| op.matches(row))?;
            let kind = op.kind?;
            Some(ContentTransfer {
                key: transfer_key(row),
                id: row.id.clone(),
                peer: row.peer.clone(),
                device_name: name(&row.peer),
                incoming: row.incoming,
                kind,
                names: op.names.clone(),
                total_bytes: row.total_bytes,
                completed_bytes: row.completed_bytes,
                prepared_bytes: 0,
                stage: content_stage(row.stage),
                error: if row.incoming
                    && s.application_failures
                        .contains(&(row.id.clone(), row.peer.clone()))
                {
                    Some(TransferFailure::Clipboard)
                } else {
                    row.error.map(transfer_failure)
                },
                saved_paths: row.saved_paths.clone(),
                at_ms: op.at_ms,
            })
        })
        .collect();
    let peers = c
        .peers
        .iter()
        .map(|(id, p)| {
            let live = n.connections.iter().find(|v| &v.peer == id);
            PeerStatus {
                noob_id: id.clone(),
                device_name: p.trusted.identity.device_name.clone(),
                fingerprint: p.trusted.identity.fingerprint.clone(),
                settings: p.settings.clone(),
                online: live.is_some_and(|v| v.connected),
                accepting: live.is_some_and(|v| v.accepting),
            }
        })
        .collect();
    let fault = s
        .fault
        .as_ref()
        .or(d.fault.as_ref())
        .or(h.error.as_ref())
        .map(|message| Fault {
            sequence: revision,
            peer: None,
            message: message.clone(),
        });
    AppSnapshot {
        session: session.into(),
        revision,
        history_revision: h.revision,
        status: Status {
            state,
            restart_required: c.settings.device_name != startup.device_name
                || c.settings.listen_address != startup.listen_address
                || c.settings.pairing_listen_address != startup.pairing_listen_address,
            configuration_revision: c.revision,
            effective_revision: s.effective_revision.min(d.effective_revision),
            noob_id: n.identity.id.clone(),
            fingerprint: n.identity.fingerprint.clone(),
            listen_address: n.listen_address.to_string(),
            settings: c.settings.clone(),
            peers,
            manual_targets: c.manual_targets.clone(),
            transfers,
        },
        content_transfers,
        local_network: d.local.clone(),
        onboarding: d.onboarding.clone(),
        current: s.current.clone(),
        activities: s.activities.iter().cloned().collect(),
        fault,
    }
}

fn transfer_failure(error: nooboard_network::ErrorKind) -> TransferFailure {
    use nooboard_network::ErrorKind;
    match error {
        ErrorKind::TooLarge => TransferFailure::TooLarge,
        ErrorKind::SourceChanged => TransferFailure::SourceChanged,
        ErrorKind::Integrity => TransferFailure::Integrity,
        ErrorKind::Unavailable | ErrorKind::Stopped => TransferFailure::Offline,
        ErrorKind::Timeout => TransferFailure::Timeout,
        ErrorKind::Busy => TransferFailure::Busy,
        ErrorKind::Cancelled => TransferFailure::Cancelled,
        ErrorKind::InvalidInput => TransferFailure::Unsupported,
        _ => TransferFailure::Protocol,
    }
}
