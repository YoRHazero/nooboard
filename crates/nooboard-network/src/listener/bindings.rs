use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use if_addrs::{IfOperStatus, Interface};
use tokio::net::TcpListener;

use crate::errors::NetworkError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListenerBindings {
    pub(crate) listen_addr: SocketAddrV4,
    pub(crate) advertise_addrs: Vec<Ipv4Addr>,
}

impl ListenerBindings {
    pub(crate) async fn bind(listen_port: u16) -> Result<(Self, TcpListener), NetworkError> {
        let listen_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, listen_port);
        let listener = TcpListener::bind(SocketAddr::V4(listen_addr))
            .await
            .map_err(|error| NetworkError::Internal(error.to_string()))?;

        let advertise_addrs = current_advertisable_ipv4_addrs();
        Ok((
            Self {
                listen_addr,
                advertise_addrs,
            },
            listener,
        ))
    }
}

pub(crate) fn current_advertisable_ipv4_addrs() -> Vec<Ipv4Addr> {
    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };

    let mut addrs: Vec<Ipv4Addr> = interfaces
        .into_iter()
        .filter(should_publish_interface)
        .filter_map(|interface| match interface.ip() {
            std::net::IpAddr::V4(ip) => Some(ip),
            std::net::IpAddr::V6(_) => None,
        })
        .collect();
    addrs.sort();
    addrs.dedup();
    addrs
}

fn should_publish_interface(interface: &Interface) -> bool {
    if interface.oper_status != IfOperStatus::Up || interface.is_loopback() || interface.is_p2p() {
        return false;
    }
    matches!(interface.ip(), std::net::IpAddr::V4(ip) if !ip.is_unspecified())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertise_addrs_are_unique() {
        let mut addrs = current_advertisable_ipv4_addrs();
        let before = addrs.len();
        addrs.sort();
        addrs.dedup();
        assert_eq!(addrs.len(), before);
    }
}
