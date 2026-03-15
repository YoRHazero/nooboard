use nooboard_core::{NetworkStatus, WorkspaceSnapshot};

#[derive(Clone)]
pub struct NetworkPageViewState {
    pub status_label: String,
    pub can_start: bool,
    pub can_stop: bool,
    pub lan_peers: Vec<String>,
    pub direct_seeds: Vec<String>,
    pub pending_requests: Vec<String>,
    pub sessions: Vec<String>,
}

pub(super) fn build_network_page_view_state(snapshot: &WorkspaceSnapshot) -> NetworkPageViewState {
    NetworkPageViewState {
        status_label: network_status_label(&snapshot.network.status),
        can_start: matches!(snapshot.network.status, NetworkStatus::Stopped),
        can_stop: matches!(
            snapshot.network.status,
            NetworkStatus::Starting | NetworkStatus::Running | NetworkStatus::Error(_)
        ),
        lan_peers: snapshot
            .network
            .lan_peers
            .iter()
            .map(|peer| {
                let address = peer
                    .addresses
                    .first()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unknown".to_string());
                format!("{} · {} · {}", peer.device_id, address, peer.noob_id)
            })
            .collect(),
        direct_seeds: snapshot
            .network
            .direct_seeds
            .iter()
            .map(|seed| {
                format!(
                    "{} · {}:{} · {}",
                    seed.label,
                    seed.host,
                    seed.port,
                    if seed.enabled { "enabled" } else { "disabled" }
                )
            })
            .collect(),
        pending_requests: snapshot
            .network
            .pending_direct_requests
            .iter()
            .map(|request| {
                format!(
                    "{} · {} · {}",
                    request.peer_device_id, request.remote_addr, request.peer_noob_id
                )
            })
            .collect(),
        sessions: snapshot
            .network
            .sessions
            .iter()
            .map(|session| {
                format!(
                    "{} · {:?} · {}",
                    session.peer_device_id, session.mode, session.remote_addr
                )
            })
            .collect(),
    }
}

pub(super) fn network_status_label(status: &NetworkStatus) -> String {
    match status {
        NetworkStatus::Stopped => "Stopped".to_string(),
        NetworkStatus::Starting => "Starting".to_string(),
        NetworkStatus::Running => "Running".to_string(),
        NetworkStatus::Error(message) => format!("Error: {message}"),
    }
}
