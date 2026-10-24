use crate::{
    Error,
    contracts::{PortFuture, terminal::*},
    features::terminal::ports::devices::DeviceAddressPort,
};
pub(crate) struct DeviceAddressAdapter(pub nooboard_network::Network);
fn unavailable() -> Error {
    TerminalError::new(
        TerminalErrorKind::AddressUnavailable,
        "SSH target has no currently available address",
    )
    .into()
}
impl DeviceAddressPort for DeviceAddressAdapter {
    fn resolve(&self, source: SshAddressSource, port: u16) -> PortFuture<'_, std::net::SocketAddr> {
        Box::pin(async move {
            match source {
                SshAddressSource::Host(host) => tokio::net::lookup_host((host.as_str(), port))
                    .await
                    .map_err(|_| unavailable())?
                    .find(|a| !a.ip().is_unspecified() && !a.ip().is_multicast())
                    .ok_or_else(unavailable),
                SshAddressSource::Device(id) => {
                    let state = self.0.status();
                    // Discovery is sufficient; APP pairing is not a prerequisite for SSH.
                    for peer in state.nearby.iter().filter(|p| p.noob_id == id.0) {
                        for address in &peer.addresses {
                            if let Ok(mut address) = address.parse::<std::net::SocketAddr>() {
                                address.set_port(port);
                                if !address.ip().is_unspecified() && !address.ip().is_multicast() {
                                    return Ok(address);
                                }
                            }
                        }
                    }
                    state
                        .connections
                        .iter()
                        .filter(|p| p.peer == id.0 && p.connected)
                        .filter_map(|p| p.peer_address)
                        .next()
                        .map(|mut a| {
                            a.set_port(port);
                            a
                        })
                        .ok_or_else(unavailable)
                }
            }
        })
    }
}
