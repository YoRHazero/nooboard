use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use crate::DirectSeedInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolveError {
    LookupFailed(String),
    NoIpv4Addresses,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LookupFailed(detail) => write!(f, "{detail}"),
            Self::NoIpv4Addresses => write!(f, "no IPv4 addresses found"),
        }
    }
}

pub(crate) async fn resolve_seed_ipv4_addrs(
    seed: &DirectSeedInfo,
) -> Result<Vec<SocketAddr>, ResolveError> {
    if let Ok(ip) = seed.host.parse::<Ipv4Addr>() {
        return Ok(vec![SocketAddr::V4(SocketAddrV4::new(ip, seed.port))]);
    }

    let resolved = tokio::net::lookup_host((seed.host.as_str(), seed.port))
        .await
        .map_err(|error| ResolveError::LookupFailed(error.to_string()))?;

    let mut seen = HashSet::new();
    let mut ipv4_addrs = Vec::new();
    for addr in resolved {
        let std::net::IpAddr::V4(ip) = addr.ip() else {
            continue;
        };
        if seen.insert(ip) {
            ipv4_addrs.push(SocketAddr::V4(SocketAddrV4::new(ip, seed.port)));
        }
    }

    if ipv4_addrs.is_empty() {
        Err(ResolveError::NoIpv4Addresses)
    } else {
        Ok(ipv4_addrs)
    }
}

#[cfg(test)]
mod tests {
    use crate::{DirectSeedId, DirectSeedInfo};

    use super::*;

    fn seed(host: &str) -> DirectSeedInfo {
        DirectSeedInfo {
            id: DirectSeedId::new(),
            label: "seed".to_string(),
            host: host.to_string(),
            port: 17890,
            enabled: true,
            learned_device_id: None,
            last_connected_addr: None,
        }
    }

    #[tokio::test]
    async fn ipv4_literal_short_circuits_lookup() {
        let addrs = resolve_seed_ipv4_addrs(&seed("127.0.0.1"))
            .await
            .expect("addr");
        assert_eq!(addrs, vec!["127.0.0.1:17890".parse().expect("addr")]);
    }
}
