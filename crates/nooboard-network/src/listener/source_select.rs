use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use crate::errors::NetworkError;

pub(crate) fn select_local_ipv4_for_remote(remote_addr: SocketAddr) -> Result<Ipv4Addr, NetworkError> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|error| NetworkError::Internal(error.to_string()))?;
    socket
        .connect(remote_addr)
        .map_err(|error| NetworkError::Internal(error.to_string()))?;
    let local_addr = socket
        .local_addr()
        .map_err(|error| NetworkError::Internal(error.to_string()))?;
    match local_addr.ip() {
        IpAddr::V4(ip) => Ok(ip),
        IpAddr::V6(_) => Err(NetworkError::Internal(
            "selected local source address is not IPv4".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_local_ipv4_for_loopback_remote() {
        let local = select_local_ipv4_for_remote("127.0.0.1:17890".parse().expect("addr"))
            .expect("local source");
        assert_eq!(local, Ipv4Addr::LOCALHOST);
    }
}
