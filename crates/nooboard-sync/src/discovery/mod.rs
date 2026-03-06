use std::net::SocketAddr;

pub mod mdns;

pub use mdns::{MdnsDiscoveryConfig, MdnsHandle, NOOBOARD_SERVICE_TYPE, start_mdns_discovery};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscoveredPeer {
    pub noob_id: String,
    pub addr: SocketAddr,
}
