use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::thread;

use mdns_sd::{
    Error as MdnsError, ResolvedService, ScopedIp, ServiceDaemon, ServiceEvent, ServiceInfo,
};
use tokio::sync::mpsc;

use crate::SyncError;

pub const MDNS_SERVICE_TYPE: &str = "_nooboard._tcp.local.";

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub service_type: String,
    pub instance_name: String,
    pub host_name: String,
    pub device_id: String,
    pub listen_addr: SocketAddr,
}

impl DiscoveryConfig {
    pub fn new(device_id: impl Into<String>, listen_addr: SocketAddr) -> Self {
        let device_id = device_id.into();
        Self {
            service_type: MDNS_SERVICE_TYPE.to_string(),
            instance_name: format!("nooboard-{device_id}"),
            host_name: format!("{device_id}.local."),
            device_id,
            listen_addr,
        }
    }
}

pub struct MdnsHandle {
    daemon: ServiceDaemon,
    _browse_thread: thread::JoinHandle<()>,
}

impl Drop for MdnsHandle {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}

pub fn start_mdns(
    config: &DiscoveryConfig,
    peer_sender: mpsc::UnboundedSender<SocketAddr>,
) -> Result<MdnsHandle, SyncError> {
    let daemon = ServiceDaemon::new().map_err(|error| SyncError::Mdns(error.to_string()))?;
    let mut properties = HashMap::new();
    properties.insert("device_id".to_string(), config.device_id.clone());
    let listen_ip_unspecified = config.listen_addr.ip().is_unspecified();
    let advertised_ip = if listen_ip_unspecified {
        String::new()
    } else {
        config.listen_addr.ip().to_string()
    };
    let service = ServiceInfo::new(
        &config.service_type,
        &config.instance_name,
        &config.host_name,
        advertised_ip.as_str(),
        config.listen_addr.port(),
        Some(properties),
    )
    .map_err(|error: MdnsError| SyncError::Mdns(error.to_string()))?;
    let service = if listen_ip_unspecified {
        // When listening on 0.0.0.0, ask mdns-sd to advertise active interface addresses.
        service.enable_addr_auto()
    } else {
        service
    };
    daemon
        .register(service)
        .map_err(|error| SyncError::Mdns(error.to_string()))?;
    let receiver = daemon
        .browse(&config.service_type)
        .map_err(|error| SyncError::Mdns(error.to_string()))?;

    let local_device_id = config.device_id.clone();
    let local_loopback_mode = config.listen_addr.ip().is_loopback();
    let browse_thread = thread::spawn(move || {
        let mut last_targets = HashMap::<String, SocketAddr>::new();
        while let Ok(event) = receiver.recv() {
            if let ServiceEvent::ServiceResolved(info) = event {
                if let Some((peer_key, peer_addr)) =
                    resolve_peer_candidate(&local_device_id, local_loopback_mode, &info)
                {
                    let changed = last_targets.get(&peer_key).copied() != Some(peer_addr);
                    if changed {
                        last_targets.insert(peer_key, peer_addr);
                        let _ = peer_sender.send(peer_addr);
                    }
                }
            }
        }
    });

    Ok(MdnsHandle {
        daemon,
        _browse_thread: browse_thread,
    })
}

fn resolve_peer_candidate(
    local_device_id: &str,
    local_loopback_mode: bool,
    info: &ResolvedService,
) -> Option<(String, SocketAddr)> {
    let device_id = info
        .get_property_val_str("device_id")
        .map(ToOwned::to_owned);
    if device_id.as_deref() == Some(local_device_id) {
        return None;
    }

    let best_ip = select_best_ip(info.get_addresses(), local_loopback_mode)?;
    let peer_key = device_id.unwrap_or_else(|| info.get_fullname().to_string());
    Some((peer_key, SocketAddr::new(best_ip, info.get_port())))
}

fn select_best_ip(
    addresses: &std::collections::HashSet<ScopedIp>,
    loopback_mode: bool,
) -> Option<IpAddr> {
    addresses
        .iter()
        .map(ScopedIp::to_ip_addr)
        .filter(|ip| should_keep_ip(*ip, loopback_mode))
        .min_by_key(|ip| (ip_rank(*ip, loopback_mode), ip.to_string()))
}

fn should_keep_ip(ip: IpAddr, loopback_mode: bool) -> bool {
    if ip.is_unspecified() || ip.is_multicast() {
        return false;
    }

    if loopback_mode {
        return ip.is_loopback();
    }

    if ip.is_loopback() {
        return false;
    }

    if let IpAddr::V6(v6) = ip {
        if v6.is_unicast_link_local() {
            return false;
        }
    }

    true
}

fn ip_rank(ip: IpAddr, loopback_mode: bool) -> u8 {
    if loopback_mode {
        return match ip {
            IpAddr::V4(v4) if v4 == Ipv4Addr::LOCALHOST => 0,
            IpAddr::V4(_) => 1,
            IpAddr::V6(_) => 2,
        };
    }

    match ip {
        IpAddr::V4(v4) if v4.is_private() => 0,
        IpAddr::V4(_) => 1,
        IpAddr::V6(v6) if v6.is_unique_local() => 2,
        IpAddr::V6(_) => 3,
    }
}
