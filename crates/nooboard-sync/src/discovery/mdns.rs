use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV6};

use mdns_sd::{ScopedIp, ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use crate::error::DiscoveryError;

use super::{DiscoveredPeer, sort_socket_addrs_by_preference};

pub const NOOBOARD_SERVICE_TYPE: &str = "_nooboard-sync._tcp.local.";
const NODE_ID_PROPERTY: &str = "noob_id";

#[derive(Debug, Clone)]
pub struct MdnsDiscoveryConfig {
    pub noob_id: String,
    pub listen_addr: SocketAddr,
    pub service_type: String,
}

impl MdnsDiscoveryConfig {
    pub fn new(noob_id: String, listen_addr: SocketAddr) -> Self {
        Self {
            noob_id,
            listen_addr,
            service_type: NOOBOARD_SERVICE_TYPE.to_string(),
        }
    }
}

pub struct MdnsHandle {
    task: JoinHandle<()>,
}

impl MdnsHandle {
    pub async fn shutdown(self) {
        let _ = self.task.await;
    }
}

pub fn start_mdns_discovery(
    config: MdnsDiscoveryConfig,
    peer_tx: mpsc::Sender<DiscoveredPeer>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<MdnsHandle, DiscoveryError> {
    let daemon = ServiceDaemon::new().map_err(|error| DiscoveryError::Mdns(error.to_string()))?;

    let service_info = build_service_info(&config)?;
    daemon
        .register(service_info)
        .map_err(|error| DiscoveryError::Mdns(error.to_string()))?;

    let receiver = daemon
        .browse(&config.service_type)
        .map_err(|error| DiscoveryError::Mdns(error.to_string()))?;

    let local_noob_id = config.noob_id;
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break;
                }
                event = receiver.recv_async() => {
                    let Ok(event) = event else {
                        break;
                    };

                    if let ServiceEvent::ServiceResolved(resolved) = event {
                        if !resolved.is_valid() {
                            continue;
                        }

                        let Some(noob_id) = resolved
                            .get_property_val_str(NODE_ID_PROPERTY)
                            .map(ToOwned::to_owned)
                        else {
                            continue;
                        };

                        if noob_id == local_noob_id {
                            continue;
                        }

                        let mut addrs: Vec<SocketAddr> = resolved
                            .get_addresses()
                            .iter()
                            .filter_map(|scoped_ip| {
                                socket_addr_from_scoped_ip(scoped_ip, resolved.get_port())
                            })
                            .collect();
                        sort_socket_addrs_by_preference(&mut addrs);

                        if addrs.is_empty() {
                            continue;
                        }

                        let peer = DiscoveredPeer {
                            noob_id: noob_id.clone(),
                            addrs,
                        };

                        if peer_tx.send(peer).await.is_err() {
                            let _ = daemon.shutdown();
                            return;
                        }
                    }
                }
            }
        }

        if let Ok(status_rx) = daemon.shutdown() {
            let _ = status_rx.recv_async().await;
        }
    });

    Ok(MdnsHandle { task })
}

fn build_service_info(config: &MdnsDiscoveryConfig) -> Result<ServiceInfo, DiscoveryError> {
    let mut instance_name = config.noob_id.clone();
    if instance_name.trim().is_empty() {
        instance_name = "nooboard".to_string();
    }

    let host_name = format!("{}.local.", sanitize_label(&instance_name));
    let addresses = local_service_addresses(config.listen_addr);
    let properties = [(NODE_ID_PROPERTY, config.noob_id.as_str())];

    ServiceInfo::new(
        &config.service_type,
        &instance_name,
        &host_name,
        addresses.as_slice(),
        config.listen_addr.port(),
        &properties[..],
    )
    .map(ServiceInfo::enable_addr_auto)
    .map_err(|error| DiscoveryError::Mdns(error.to_string()))
}

fn socket_addr_from_scoped_ip(scoped_ip: &ScopedIp, port: u16) -> Option<SocketAddr> {
    match scoped_ip {
        ScopedIp::V4(v4) => {
            let ip = *v4.addr();
            if ip.is_unspecified() {
                return None;
            }
            Some(SocketAddr::new(IpAddr::V4(ip), port))
        }
        ScopedIp::V6(v6) => {
            let ip = *v6.addr();
            if ip.is_unspecified() {
                return None;
            }

            let scope_id = if ip.is_unicast_link_local() {
                let scope_id = v6.scope_id().index;
                if scope_id == 0 {
                    debug!("skip link-local IPv6 mDNS address without scope: {ip}");
                    return None;
                }
                scope_id
            } else {
                0
            };

            Some(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, scope_id)))
        }
        _ => None,
    }
}

fn local_service_addresses(listen_addr: SocketAddr) -> Vec<IpAddr> {
    if !listen_addr.ip().is_unspecified() {
        return vec![listen_addr.ip()];
    }

    let mut addresses: Vec<IpAddr> = if_addrs::get_if_addrs()
        .map(|interfaces| {
            interfaces
                .into_iter()
                .map(|interface| interface.ip())
                .filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
                .collect()
        })
        .unwrap_or_default();

    if addresses.is_empty() {
        warn!("failed to detect non-loopback interface address for mDNS, fallback to localhost");
        addresses.push(IpAddr::V4(Ipv4Addr::LOCALHOST));
    }

    addresses.sort();
    addresses.dedup();
    debug!("mDNS advertise addresses: {:?}", addresses);
    addresses
}

fn sanitize_label(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }

    let out = out.trim_matches('-');
    if out.is_empty() {
        "nooboard".to_string()
    } else {
        out.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;
    use crate::discovery::sort_socket_addrs_by_preference;

    #[test]
    fn sanitize_label_replaces_non_ascii() {
        assert_eq!(sanitize_label("node-1"), "node-1");
        assert_eq!(sanitize_label("node 1"), "node-1");
        assert_eq!(sanitize_label("中文"), "nooboard");
    }

    #[test]
    fn address_preference_prefers_routed_ipv4_before_link_local_ipv6() {
        let mut addrs = vec![
            SocketAddr::V6(SocketAddrV6::new(
                Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1),
                17890,
                0,
                7,
            )),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 44)), 17890),
            SocketAddr::V6(SocketAddrV6::new(
                Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1),
                17890,
                0,
                0,
            )),
        ];

        sort_socket_addrs_by_preference(&mut addrs);

        assert_eq!(
            addrs,
            vec![
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 44)), 17890),
                SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1),
                    17890,
                    0,
                    0,
                )),
                SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1),
                    17890,
                    0,
                    7,
                )),
            ]
        );
    }
}
