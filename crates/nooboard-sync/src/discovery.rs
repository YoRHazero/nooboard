use std::collections::HashMap;
use std::net::SocketAddr;
use std::thread;

use mdns_sd::{Error as MdnsError, ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};
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
    let advertised_ip = if config.listen_addr.ip().is_unspecified() {
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
    daemon
        .register(service)
        .map_err(|error| SyncError::Mdns(error.to_string()))?;
    let receiver = daemon
        .browse(&config.service_type)
        .map_err(|error| SyncError::Mdns(error.to_string()))?;

    let local_device_id = config.device_id.clone();
    let browse_thread = thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            if let ServiceEvent::ServiceResolved(info) = event {
                handle_resolved_peer(&local_device_id, &peer_sender, &info);
            }
        }
    });

    Ok(MdnsHandle {
        daemon,
        _browse_thread: browse_thread,
    })
}

fn handle_resolved_peer(
    local_device_id: &str,
    peer_sender: &mpsc::UnboundedSender<SocketAddr>,
    info: &ResolvedService,
) {
    let device_id = info
        .get_property_val_str("device_id")
        .map(ToOwned::to_owned);
    if device_id.as_deref() == Some(local_device_id) {
        return;
    }

    for address in info.get_addresses() {
        let addr = SocketAddr::new(address.to_ip_addr(), info.get_port());
        let _ = peer_sender.send(addr);
    }
}
