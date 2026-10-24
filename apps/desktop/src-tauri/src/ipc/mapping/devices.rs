use crate::ipc::{contracts::*, errors};
pub fn local_device(
    status: &nooboard_core::Status,
    network: nooboard_core::LocalNetwork,
) -> LocalDevice {
    LocalDevice {
        noob_id: status.noob_id.clone(),
        device_name: status.settings.device_name.clone(),
        fingerprint: status.fingerprint.clone(),
        platform: match std::env::consts::OS {
            "macos" => "macOS",
            "windows" => "Windows",
            _ => "Linux",
        }
        .into(),
        sync_port: network.sync_port,
        pairing_port: network.pairing_port,
        addresses: network
            .addresses
            .into_iter()
            .map(|a| LocalAddress {
                interface: a.interface,
                ip: a.ip.to_string(),
                pairing_address: a.pairing_address.to_string(),
            })
            .collect(),
        address_error: network.error.map(|_| errors::ui("localAddress")),
    }
}
pub fn peers(peers: Vec<nooboard_core::PeerStatus>) -> Vec<Peer> {
    peers
        .into_iter()
        .map(|p| Peer {
            noob_id: p.noob_id,
            device_name: p.device_name,
            fingerprint: p.fingerprint,
            settings: PeerSettings {
                address: p.settings.address,
                auto_send: p.settings.auto_send,
            },
            online: p.online,
            connection_address: p.connection_address,
            accepting: p.accepting,
            connection_state: p.connection_state.into(),
            paused: p.paused,
            unpaired_notice: p.unpaired_notice.map(|v| v.to_string()),
        })
        .collect()
}
pub fn onboarding(onboarding: nooboard_core::OnboardingSnapshot) -> Onboarding {
    Onboarding {
        nearby: onboarding
            .nearby
            .into_iter()
            .map(|p| NearbyDevice {
                key: p.key,
                noob_id: p.noob_id,
                device_name: p.device_name,
                addresses: p.addresses.into_iter().map(|a| a.to_string()).collect(),
                sync_port: p.sync_port,
            })
            .collect(),
        error: onboarding.discovery_error.map(|_| errors::ui("discovery")),
        session: onboarding.session.map(|p| PairingSession {
            id: p.id,
            incoming: p.incoming,
            device_name: p.device_name,
            noob_id: p.noob_id,
            stage: p.stage.into(),
            code: p.code,
            expires_at: p.expires_at_ms,
            attempts_left: p.attempts_left,
            error: p.error.as_ref().map(errors::pairing),
        }),
    }
}
