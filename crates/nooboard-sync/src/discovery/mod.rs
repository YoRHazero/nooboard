use std::cmp::Ordering;
use std::net::{IpAddr, SocketAddr};

pub mod mdns;

pub use mdns::{MdnsDiscoveryConfig, MdnsHandle, NOOBOARD_SERVICE_TYPE, start_mdns_discovery};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscoveredPeer {
    pub noob_id: String,
    pub addrs: Vec<SocketAddr>,
}

pub(crate) fn sort_socket_addrs_by_preference(addrs: &mut Vec<SocketAddr>) {
    addrs.sort_by(compare_socket_addrs);
    addrs.dedup();
}

fn compare_socket_addrs(left: &SocketAddr, right: &SocketAddr) -> Ordering {
    socket_addr_preference(left)
        .cmp(&socket_addr_preference(right))
        .then_with(|| left.to_string().cmp(&right.to_string()))
}

fn socket_addr_preference(addr: &SocketAddr) -> u8 {
    match addr.ip() {
        IpAddr::V4(ip) if ip.is_private() => 0,
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() => 1,
        IpAddr::V6(ip) if !ip.is_loopback() && !ip.is_unicast_link_local() => 2,
        IpAddr::V4(ip) if ip.is_loopback() => 3,
        IpAddr::V4(_) => 4,
        IpAddr::V6(ip) if ip.is_loopback() => 5,
        IpAddr::V6(_) => 6,
    }
}
