use nooboard_core::{DirectRequestId, DirectSeedId, NetworkStatus, SessionId, WorkspaceSnapshot};
use time::{OffsetDateTime, UtcOffset};

#[derive(Clone)]
pub struct NetworkPageViewState {
    pub status_label: String,
    pub network_enabled: bool,
    pub local_device_id: String,
    pub local_noob_id: String,
    pub network_token: String,
    pub endpoint_label: String,
    pub lan_enabled: bool,
    pub direct_seed_count: usize,
    pub pending_request_count: usize,
    pub direct_session_count: usize,
    pub connected_lan_peer_count: usize,
    pub direct_seeds: Vec<NetworkDirectSeedViewState>,
    pub pending_requests: Vec<NetworkPendingRequestViewState>,
    pub direct_sessions: Vec<NetworkSessionViewState>,
    pub lan_peers: Vec<NetworkLanPeerViewState>,
}

#[derive(Clone)]
pub struct NetworkLanPeerViewState {
    pub device_id: String,
    pub noob_id: String,
    pub endpoint_label: String,
    pub last_seen_label: String,
    pub connected: bool,
    pub connected_at_label: Option<String>,
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
    pub remote_addr_label: String,
    pub local_bind_addr_label: Option<String>,
    pub connected_at_label: String,
}

pub(super) fn build_network_page_view_state(snapshot: &WorkspaceSnapshot) -> NetworkPageViewState {
    let direct_sessions = snapshot
        .network
        .sessions
        .iter()
        .filter(|session| session_mode_is_direct(&session.mode))
        .map(|session| NetworkSessionViewState {
            id: session.id,
            peer_device_id: session.peer_device_id.clone(),
            peer_noob_id: session.peer_noob_id.clone(),
            remote_addr_label: session.remote_addr.to_string(),
            local_bind_addr_label: session.local_bind_addr.map(|value| value.to_string()),
            connected_at_label: clock_label_from_millis(session.connected_at_ms),
        })
        .collect::<Vec<_>>();

    let lan_peers = snapshot
        .network
        .lan_peers
        .iter()
        .map(|peer| {
            let connected_at_label = snapshot
                .network
                .sessions
                .iter()
                .find(|session| {
                    !session_mode_is_direct(&session.mode) && session.peer_noob_id == peer.noob_id
                })
                .map(|session| clock_label_from_millis(session.connected_at_ms));

            NetworkLanPeerViewState {
                device_id: peer.device_id.clone(),
                noob_id: peer.noob_id.clone(),
                endpoint_label: peer
                    .addresses
                    .first()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unknown endpoint".to_string()),
                last_seen_label: clock_label_from_millis(peer.last_seen_at_ms),
                connected: peer.connected,
                connected_at_label,
            }
        })
        .collect::<Vec<_>>();

    NetworkPageViewState {
        status_label: network_status_label(&snapshot.network.status),
        network_enabled: !matches!(snapshot.network.status, NetworkStatus::Stopped),
        local_device_id: snapshot.identity.device_id.clone(),
        local_noob_id: snapshot.identity.noob_id.to_string(),
        network_token: snapshot.settings.connection.token.clone(),
        endpoint_label: snapshot
            .local_connection
            .device_endpoint
            .map(|value| value.to_string())
            .unwrap_or_else(|| "not bound".to_string()),
        lan_enabled: snapshot.settings.network.lan_enabled,
        direct_seed_count: snapshot.network.direct_seeds.len(),
        pending_request_count: snapshot.network.pending_direct_requests.len(),
        direct_session_count: direct_sessions.len(),
        connected_lan_peer_count: lan_peers.iter().filter(|peer| peer.connected).count(),
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
        direct_sessions,
        lan_peers,
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

fn session_mode_is_direct(mode: &impl std::fmt::Debug) -> bool {
    format!("{mode:?}") == "Direct"
}
