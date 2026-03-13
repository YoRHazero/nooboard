use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV6};

use if_addrs::{IfOperStatus, Interface};
use mdns_sd::{IfKind, ScopedIp, ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use crate::error::DiscoveryError;

use super::{DiscoveredPeer, sort_socket_addrs_by_preference};

pub const NOOBOARD_SERVICE_TYPE: &str = "_nooboard-sync._tcp.local.";
const NODE_ID_PROPERTY: &str = "noob_id";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddressFamily {
    V4,
    V6,
}

#[derive(Debug, Clone)]
struct LocalMdnsTargets {
    addresses: Vec<IpAddr>,
    interfaces: Vec<IfKind>,
}

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
    let targets = local_mdns_targets(config.listen_addr);
    let properties = [(NODE_ID_PROPERTY, config.noob_id.as_str())];

    let mut service_info = ServiceInfo::new(
        &config.service_type,
        &instance_name,
        &host_name,
        targets.addresses.as_slice(),
        config.listen_addr.port(),
        &properties[..],
    )
    .map(ServiceInfo::enable_addr_auto)
    .map_err(|error| DiscoveryError::Mdns(error.to_string()))?;

    if !targets.interfaces.is_empty() {
        service_info.set_interfaces(targets.interfaces);
    }

    Ok(service_info)
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

fn local_mdns_targets(listen_addr: SocketAddr) -> LocalMdnsTargets {
    let family = listen_addr_family(listen_addr);

    if !listen_addr.ip().is_unspecified() {
        return LocalMdnsTargets {
            addresses: vec![listen_addr.ip()],
            interfaces: vec![IfKind::Addr(listen_addr.ip())],
        };
    }

    let mut targets = if_addrs::get_if_addrs()
        .map(|interfaces| select_local_mdns_targets(interfaces, family))
        .unwrap_or_else(|_| LocalMdnsTargets {
            addresses: Vec::new(),
            interfaces: Vec::new(),
        });

    if targets.addresses.is_empty() {
        warn!(
            "failed to detect active non-loopback {family:?} interface address for mDNS, fallback to localhost"
        );
        return localhost_mdns_targets(family);
    }

    targets.addresses.sort();
    targets.addresses.dedup();
    debug!(
        "mDNS advertise addresses: {:?}, interfaces: {}",
        targets.addresses,
        format_mdns_interface_names(&targets.interfaces)
    );
    targets
}

fn select_local_mdns_targets(
    interfaces: Vec<Interface>,
    family: AddressFamily,
) -> LocalMdnsTargets {
    let mut addresses = Vec::new();
    let mut supported_interfaces = Vec::new();

    for interface in interfaces {
        if !should_publish_interface(&interface, family) {
            continue;
        }

        let ip = interface.ip();
        addresses.push(ip);
        supported_interfaces.push(IfKind::Name(interface.name));
    }

    LocalMdnsTargets {
        addresses,
        interfaces: supported_interfaces,
    }
}

fn should_publish_interface(interface: &Interface, family: AddressFamily) -> bool {
    if interface.oper_status != IfOperStatus::Up || interface.is_loopback() || interface.is_p2p() {
        return false;
    }

    let ip = interface.ip();
    if ip.is_unspecified() {
        return false;
    }

    match family {
        AddressFamily::V4 => ip.is_ipv4(),
        AddressFamily::V6 => ip.is_ipv6(),
    }
}

fn listen_addr_family(listen_addr: SocketAddr) -> AddressFamily {
    match listen_addr {
        SocketAddr::V4(_) => AddressFamily::V4,
        SocketAddr::V6(_) => AddressFamily::V6,
    }
}

fn localhost_mdns_targets(family: AddressFamily) -> LocalMdnsTargets {
    match family {
        AddressFamily::V4 => LocalMdnsTargets {
            addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            interfaces: vec![IfKind::LoopbackV4],
        },
        AddressFamily::V6 => LocalMdnsTargets {
            addresses: vec![IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)],
            interfaces: vec![IfKind::LoopbackV6],
        },
    }
}

fn format_mdns_interface_names(interfaces: &[IfKind]) -> String {
    let mut names = Vec::new();
    for interface in interfaces {
        match interface {
            IfKind::Name(name) => names.push(name.clone()),
            IfKind::Addr(addr) => names.push(addr.to_string()),
            IfKind::IndexV4(index) | IfKind::IndexV6(index) => names.push(index.to_string()),
            IfKind::IPv4 => names.push("ipv4".to_string()),
            IfKind::IPv6 => names.push("ipv6".to_string()),
            IfKind::LoopbackV4 => names.push("loopback-v4".to_string()),
            IfKind::LoopbackV6 => names.push("loopback-v6".to_string()),
            IfKind::All => names.push("all".to_string()),
            _ => names.push("unknown".to_string()),
        }
    }
    names.join(",")
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

    #[test]
    fn ipv4_listener_only_publishes_active_ipv4_interfaces() {
        let targets = select_local_mdns_targets(
            vec![
                test_interface_v4("en0", Ipv4Addr::new(100, 64, 5, 22), true, false),
                test_interface_v6(
                    "awdl0",
                    Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1),
                    true,
                    false,
                ),
                test_interface_v4("utun0", Ipv4Addr::new(100, 115, 92, 1), true, true),
                test_interface_v4("en1", Ipv4Addr::new(192, 168, 0, 12), false, false),
            ],
            AddressFamily::V4,
        );

        assert_eq!(
            targets.addresses,
            vec![IpAddr::V4(Ipv4Addr::new(100, 64, 5, 22))]
        );
        assert_eq!(targets.interfaces.len(), 1);
        assert!(matches!(&targets.interfaces[0], IfKind::Name(name) if name == "en0"));
    }

    #[test]
    fn explicit_listen_addr_publishes_only_that_address() {
        let targets = local_mdns_targets("192.168.1.44:17890".parse().expect("valid addr"));

        assert_eq!(
            targets.addresses,
            vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 44))]
        );
        assert_eq!(targets.interfaces.len(), 1);
        assert!(matches!(
            &targets.interfaces[0],
            IfKind::Addr(IpAddr::V4(ip)) if *ip == Ipv4Addr::new(192, 168, 1, 44)
        ));
    }

    fn test_interface_v4(name: &str, ip: Ipv4Addr, up: bool, p2p: bool) -> Interface {
        Interface {
            name: name.to_string(),
            addr: if_addrs::IfAddr::V4(if_addrs::Ifv4Addr {
                ip,
                netmask: Ipv4Addr::new(255, 255, 255, 0),
                prefixlen: 24,
                broadcast: None,
            }),
            index: Some(1),
            oper_status: if up {
                IfOperStatus::Up
            } else {
                IfOperStatus::Down
            },
            is_p2p: p2p,
        }
    }

    fn test_interface_v6(name: &str, ip: Ipv6Addr, up: bool, p2p: bool) -> Interface {
        Interface {
            name: name.to_string(),
            addr: if_addrs::IfAddr::V6(if_addrs::Ifv6Addr {
                ip,
                netmask: Ipv6Addr::UNSPECIFIED,
                prefixlen: 64,
                broadcast: None,
            }),
            index: Some(1),
            oper_status: if up {
                IfOperStatus::Up
            } else {
                IfOperStatus::Down
            },
            is_p2p: p2p,
        }
    }
}
