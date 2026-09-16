//! Local interface addresses suitable for both pairing and subsequent synchronization.
use if_addrs::Interface;
use serde::Serialize;
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LocalAddress {
    pub interface: String,
    pub ip: IpAddr,
    pub pairing_address: SocketAddr,
}

/// Only report active addresses covered by both listeners. These are candidates,
/// not a reachability check: the remote machine still needs a route and firewall access.
pub fn pairing_addresses(
    pairing: SocketAddr,
    sync: SocketAddr,
) -> std::io::Result<Vec<LocalAddress>> {
    Ok(select(if_addrs::get_if_addrs()?, pairing, sync))
}

fn covers(listener: SocketAddr, ip: IpAddr) -> bool {
    // IPv4 listeners do not accept IPv6. Be conservative for IPv6 listeners too:
    // dual-stack behavior differs between platforms and socket configurations.
    listener.is_ipv4() == ip.is_ipv4() && (listener.ip().is_unspecified() || listener.ip() == ip)
}

fn select(interfaces: Vec<Interface>, pairing: SocketAddr, sync: SocketAddr) -> Vec<LocalAddress> {
    let mut addresses: Vec<_> = interfaces
        .into_iter()
        .filter(|interface| {
            let ip = interface.ip();
            interface.is_oper_up()
                && !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_multicast()
                // An IPv6 link-local address needs the remote machine's scope ID,
                // which cannot be supplied by this machine as a copyable endpoint.
                && !matches!(ip, IpAddr::V6(v6) if v6.is_unicast_link_local())
                && !matches!(ip, IpAddr::V4(v4) if v4.is_broadcast())
                && covers(pairing, ip)
                && covers(sync, ip)
        })
        .map(|interface| LocalAddress {
            ip: interface.ip(),
            pairing_address: SocketAddr::new(interface.ip(), pairing.port()),
            interface: interface.name,
        })
        .collect();
    addresses.sort_by(|a, b| (&a.ip, &a.interface).cmp(&(&b.ip, &b.interface)));
    addresses.dedup_by_key(|address| address.ip);
    addresses
}

#[cfg(test)]
mod tests {
    use super::*;
    use if_addrs::{IfAddr, IfOperStatus, Ifv4Addr, Ifv6Addr};

    fn interface(name: &str, ip: &str, up: bool) -> Interface {
        Interface {
            name: name.into(),
            addr: match ip.parse().unwrap() {
                IpAddr::V4(ip) => IfAddr::V4(Ifv4Addr {
                    ip,
                    netmask: "255.255.255.0".parse().unwrap(),
                    broadcast: None,
                    prefixlen: 24,
                }),
                IpAddr::V6(ip) => IfAddr::V6(Ifv6Addr {
                    ip,
                    netmask: "ffff:ffff:ffff:ffff::".parse().unwrap(),
                    broadcast: None,
                    prefixlen: 64,
                }),
            },
            index: None,
            oper_status: if up {
                IfOperStatus::Up
            } else {
                IfOperStatus::Down
            },
            is_p2p: false,
            #[cfg(windows)]
            adapter_name: name.into(),
        }
    }

    #[test]
    fn ipv4_candidates_include_lan_and_vpn_but_not_unusable_addresses() {
        let addresses = select(
            vec![
                interface("lan", "192.168.1.4", true),
                interface("duplicate", "192.168.1.4", true),
                interface("vpn", "100.80.2.3", true),
                interface("loopback", "127.0.0.1", true),
                interface("offline", "10.0.0.2", false),
                interface("wildcard", "0.0.0.0", true),
                interface("multicast", "224.0.0.251", true),
                interface("broadcast", "255.255.255.255", true),
                interface("v6", "fd00::1", true),
            ],
            "0.0.0.0:24817".parse().unwrap(),
            "0.0.0.0:24816".parse().unwrap(),
        );
        assert_eq!(addresses.len(), 2);
        assert_eq!(addresses[0].pairing_address.to_string(), "100.80.2.3:24817");
        assert_eq!(
            addresses[1].pairing_address.to_string(),
            "192.168.1.4:24817"
        );
    }

    #[test]
    fn both_listeners_must_cover_the_address() {
        let interfaces = vec![interface("lan", "192.168.1.4", true)];
        for (pairing, sync) in [
            ("127.0.0.1:1234", "0.0.0.0:1235"),
            ("0.0.0.0:1234", "192.168.1.5:1235"),
        ] {
            assert!(
                select(
                    interfaces.clone(),
                    pairing.parse().unwrap(),
                    sync.parse().unwrap()
                )
                .is_empty()
            );
        }
        assert_eq!(
            select(
                interfaces,
                "192.168.1.4:1234".parse().unwrap(),
                "0.0.0.0:1235".parse().unwrap()
            )
            .len(),
            1
        );
    }

    #[test]
    fn ipv6_candidates_are_bracketed_and_exclude_local_scope() {
        let addresses = select(
            vec![
                interface("v6", "fd00::1", true),
                interface("link", "fe80::1", true),
                interface("lo", "::1", true),
            ],
            "[::]:30000".parse().unwrap(),
            "[::]:30001".parse().unwrap(),
        );
        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0].pairing_address.to_string(), "[fd00::1]:30000");
    }
}
