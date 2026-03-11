use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub(super) fn detect_device_endpoint(listen_port: u16) -> Option<SocketAddr> {
    let ipv4_addresses = if_addrs::get_if_addrs().ok().map(|interfaces| {
        interfaces
            .into_iter()
            .filter_map(|interface| match interface.ip() {
                IpAddr::V4(ipv4) => Some(ipv4),
                IpAddr::V6(_) => None,
            })
            .collect::<Vec<_>>()
    })?;

    select_device_ip(ipv4_addresses).map(|ipv4| SocketAddr::new(IpAddr::V4(ipv4), listen_port))
}

fn select_device_ip(addresses: Vec<Ipv4Addr>) -> Option<Ipv4Addr> {
    let mut private = None;
    let mut non_loopback = None;
    let mut loopback = None;

    for ipv4 in addresses {
        if ipv4.is_loopback() {
            loopback.get_or_insert(ipv4);
            continue;
        }

        if ipv4.is_private() {
            private.get_or_insert(ipv4);
        }
        non_loopback.get_or_insert(ipv4);
    }

    private.or(non_loopback).or(loopback)
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::select_device_ip;

    #[test]
    fn prefers_first_private_non_loopback_ipv4() {
        let selected = select_device_ip(vec![
            Ipv4Addr::LOCALHOST,
            Ipv4Addr::new(8, 8, 8, 8),
            Ipv4Addr::new(192, 168, 1, 44),
            Ipv4Addr::new(10, 0, 0, 5),
        ]);

        assert_eq!(selected, Some(Ipv4Addr::new(192, 168, 1, 44)));
    }

    #[test]
    fn falls_back_to_first_non_loopback_ipv4() {
        let selected = select_device_ip(vec![
            Ipv4Addr::LOCALHOST,
            Ipv4Addr::new(8, 8, 4, 4),
            Ipv4Addr::new(1, 1, 1, 1),
        ]);

        assert_eq!(selected, Some(Ipv4Addr::new(8, 8, 4, 4)));
    }

    #[test]
    fn falls_back_to_loopback_ipv4() {
        let selected = select_device_ip(vec![Ipv4Addr::LOCALHOST]);

        assert_eq!(selected, Some(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn returns_none_without_any_ipv4_address() {
        assert_eq!(select_device_ip(Vec::new()), None);
    }

    #[test]
    fn preserves_the_requested_port_in_device_endpoint() {
        let endpoint = super::detect_device_endpoint(17890)
            .map(|endpoint| SocketAddr::new(endpoint.ip(), endpoint.port()));

        if let Some(endpoint) = endpoint {
            assert_eq!(endpoint.port(), 17890);
            assert!(matches!(endpoint.ip(), IpAddr::V4(_)));
        }
    }
}
