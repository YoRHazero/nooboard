use nooboard_core::{DirectRequestId, DirectSeedId, NetworkStatus, SessionId, WorkspaceSnapshot};
use time::{OffsetDateTime, UtcOffset};

#[derive(Clone)]
pub struct NetworkPageViewState {
    pub status_label: String,
    pub can_start: bool,
    pub can_stop: bool,
    pub lan_peer_count: usize,
    pub direct_seed_count: usize,
    pub pending_request_count: usize,
    pub session_count: usize,
    pub lan_peers: Vec<NetworkLanPeerViewState>,
    pub direct_seeds: Vec<NetworkDirectSeedViewState>,
    pub pending_requests: Vec<NetworkPendingRequestViewState>,
    pub sessions: Vec<NetworkSessionViewState>,
}

#[derive(Clone)]
pub struct NetworkLanPeerViewState {
    pub device_id: String,
    pub noob_id: String,
    pub endpoint_label: String,
    pub last_seen_label: String,
    pub connected: bool,
}

#[derive(Clone)]
pub struct NetworkDirectSeedViewState {
    pub id: DirectSeedId,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub endpoint_label: String,
    pub enabled: bool,
    pub learned_device_id: Option<String>,
    pub last_connected_addr_label: Option<String>,
}

#[derive(Clone)]
pub struct NetworkPendingRequestViewState {
    pub id: DirectRequestId,
    pub peer_device_id: String,
    pub peer_noob_id: String,
    pub remote_addr_label: String,
    pub expires_label: String,
}

#[derive(Clone)]
pub struct NetworkSessionViewState {
    pub id: SessionId,
    pub peer_device_id: String,
    pub peer_noob_id: String,
    pub mode_label: String,
    pub remote_addr_label: String,
    pub local_bind_addr_label: Option<String>,
    pub connected_at_label: String,
}

pub(super) fn build_network_page_view_state(snapshot: &WorkspaceSnapshot) -> NetworkPageViewState {
    NetworkPageViewState {
        status_label: network_status_label(&snapshot.network.status),
        can_start: matches!(snapshot.network.status, NetworkStatus::Stopped),
        can_stop: matches!(
            snapshot.network.status,
            NetworkStatus::Starting | NetworkStatus::Running | NetworkStatus::Error(_)
        ),
        lan_peer_count: snapshot.network.lan_peers.len(),
        direct_seed_count: snapshot.network.direct_seeds.len(),
        pending_request_count: snapshot.network.pending_direct_requests.len(),
        session_count: snapshot.network.sessions.len(),
        lan_peers: snapshot
            .network
            .lan_peers
            .iter()
            .map(|peer| NetworkLanPeerViewState {
                device_id: peer.device_id.clone(),
                noob_id: peer.noob_id.clone(),
                endpoint_label: peer
                    .addresses
                    .first()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unknown endpoint".to_string()),
                last_seen_label: clock_label_from_millis(peer.last_seen_at_ms),
                connected: peer.connected,
            })
            .collect(),
        direct_seeds: snapshot
            .network
            .direct_seeds
            .iter()
            .map(|seed| NetworkDirectSeedViewState {
                id: seed.id,
                label: seed.label.clone(),
                host: seed.host.clone(),
                port: seed.port,
                endpoint_label: format!("{}:{}", seed.host, seed.port),
                enabled: seed.enabled,
                learned_device_id: seed.learned_device_id.clone(),
                last_connected_addr_label: seed.last_connected_addr.map(|value| value.to_string()),
            })
            .collect(),
        pending_requests: snapshot
            .network
            .pending_direct_requests
            .iter()
            .map(|request| NetworkPendingRequestViewState {
                id: request.id,
                peer_device_id: request.peer_device_id.clone(),
                peer_noob_id: request.peer_noob_id.clone(),
                remote_addr_label: request.remote_addr.to_string(),
                expires_label: clock_label_from_millis(request.expires_at_ms),
            })
            .collect(),
        sessions: snapshot
            .network
            .sessions
            .iter()
            .map(|session| NetworkSessionViewState {
                id: session.id,
                peer_device_id: session.peer_device_id.clone(),
                peer_noob_id: session.peer_noob_id.clone(),
                mode_label: format!("{:?}", session.mode),
                remote_addr_label: session.remote_addr.to_string(),
                local_bind_addr_label: session.local_bind_addr.map(|value| value.to_string()),
                connected_at_label: clock_label_from_millis(session.connected_at_ms),
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

fn clock_label_from_millis(timestamp_ms: u64) -> String {
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let nanos = i128::from(timestamp_ms) * 1_000_000;
    let datetime = OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .to_offset(offset);

    format!(
        "{:02}:{:02}:{:02}",
        datetime.hour(),
        datetime.minute(),
        datetime.second()
    )
}
